//! Lift nested SL calls and wrap their owning instruction in let bindings
//!
//! `lift_instr` repeatedly removes the leftmost eligible call from one
//! instruction. Expression and path dispatchers preserve the iteration state
//! needed to rebuild the surrounding let instructions.

use crate::lang::{
    common::{
        ds::{map::IdMap, set::IdSet},
        source::Span,
    },
    hints::input,
    il::{self, ast as il_ast},
    sl::ast as sl,
    traits::{eq::SyntaxEq, free::FreeVars},
};

use super::super::{ProseError, ProseErrorKind};

// == Call lifting

/// Tracks call nesting along one expression path.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum CallNesting {
    None,
    Outer,
    Nested,
}

impl CallNesting {
    /// Updates the nesting state at an expression boundary.
    fn enter_exp(self, exp_kind_sl: &sl::ExpKind) -> Self {
        match exp_kind_sl {
            il_ast::ExpKind::Call(_, _, _) => match self {
                Self::None => Self::Outer,
                Self::Outer | Self::Nested => Self::Nested,
            },
            il_ast::ExpKind::Iter(_, _) => self,
            _ => Self::Nested,
        }
    }
}

/// Describes one call removed from an expression and its replacement binding.
struct LiftedCall {
    exp_new_sl: sl::Exp,
    exp_call_sl: sl::Exp,
    vars_call_sl: Vec<sl::Var>,
    var_new_sl: sl::Var,
    iter_exps_enclosing_sl: Vec<sl::ExpIter>,
}

impl LiftedCall {
    /// Wraps an instruction in the let binding for this lifted call.
    fn wrap_instr(self, instr_sl: sl::Instr) -> sl::Instr {
        let Self { exp_new_sl, exp_call_sl, var_new_sl, iter_exps_enclosing_sl, .. } = self;
        // Separate callee dimensions from iterations crossed during lifting
        let num_iters_enclosing = iter_exps_enclosing_sl.len();
        let num_iters_callee = var_new_sl.iters.len() - num_iters_enclosing;
        let mut var_bind_sl = sl::Var {
            id: var_new_sl.id,
            typ: var_new_sl.typ,
            iters: var_new_sl.iters[..num_iters_callee].to_vec(),
        };

        // Rebuild instruction iterations from the inside out
        let mut iter_instrs_sl = Vec::new();
        for (iter, vars_bound_sl) in iter_exps_enclosing_sl {
            iter_instrs_sl.push(sl::InstrIter {
                iter,
                vars_bound: vars_bound_sl,
                vars_bind: vec![var_bind_sl.clone()],
            });
            var_bind_sl.iters.push(iter);
        }

        // Bind the extracted call immediately before its owning instruction
        let span = instr_sl.span.clone();
        crate::phrase! {
            node: sl::InstrKind::Let(sl::LetInstr {
                exp_l: exp_new_sl,
                exp_r: exp_call_sl,
                iter_instrs: iter_instrs_sl,
                block: vec![instr_sl],
            }),
            span: span,
        }
    }
}

// == Expressions

// - Expression

/// Lifts the leftmost nested call from an expression.
fn lift_from_exp(
    ids_used: &mut IdSet,
    nesting: CallNesting,
    exp_target_sl: &mut sl::Exp,
) -> Option<LiftedCall> {
    let nesting = nesting.enter_exp(&exp_target_sl.node);
    if nesting == CallNesting::Nested
        && let Some(call_lifted) = try_lift_call(ids_used, exp_target_sl)
    {
        return Some(call_lifted);
    }
    // Continue into children in source order
    lift_exp_kind(ids_used, nesting, &mut exp_target_sl.node)
}

fn lift_exp_kind(
    ids_used: &mut IdSet,
    nesting: CallNesting,
    exp_kind_sl: &mut sl::ExpKind,
) -> Option<LiftedCall> {
    match exp_kind_sl {
        il_ast::ExpKind::Bool(_)
        | il_ast::ExpKind::Num(_)
        | il_ast::ExpKind::Text(_)
        | il_ast::ExpKind::Var(_) => None,
        il_ast::ExpKind::Un(_, _, exp_inner_sl)
        | il_ast::ExpKind::UpCast(_, exp_inner_sl)
        | il_ast::ExpKind::DownCast(_, exp_inner_sl)
        | il_ast::ExpKind::Sub(exp_inner_sl, _, _)
        | il_ast::ExpKind::Match(exp_inner_sl, _)
        | il_ast::ExpKind::Len(exp_inner_sl)
        | il_ast::ExpKind::Dot(exp_inner_sl, _) => lift_from_exp(ids_used, nesting, exp_inner_sl),
        il_ast::ExpKind::Bin(_, _, exp_l_sl, exp_r_sl)
        | il_ast::ExpKind::Cmp(_, _, exp_l_sl, exp_r_sl)
        | il_ast::ExpKind::Cons(exp_l_sl, exp_r_sl)
        | il_ast::ExpKind::Cat(exp_l_sl, exp_r_sl)
        | il_ast::ExpKind::Mem(exp_l_sl, exp_r_sl)
        | il_ast::ExpKind::Idx(exp_l_sl, exp_r_sl) => {
            lift_binary_exp(ids_used, nesting, exp_l_sl, exp_r_sl)
        }
        il_ast::ExpKind::Tuple(exps_sl) | il_ast::ExpKind::List(exps_sl) => {
            lift_exps(ids_used, nesting, exps_sl)
        }
        il_ast::ExpKind::Case(not_exp_sl) => lift_case_exp(ids_used, nesting, not_exp_sl),
        il_ast::ExpKind::Str(fields_sl) => lift_struct_exp(ids_used, nesting, fields_sl),
        il_ast::ExpKind::Opt(Some(exp_sl)) => lift_from_exp(ids_used, nesting, exp_sl),
        il_ast::ExpKind::Opt(None) => None,
        il_ast::ExpKind::Slice(exp_base_sl, exp_idx_sl, exp_len_sl) => {
            lift_slice_exp(ids_used, nesting, exp_base_sl, exp_idx_sl, exp_len_sl)
        }
        il_ast::ExpKind::Upd(exp_base_sl, path_sl, exp_field_sl) => {
            lift_update_exp(ids_used, nesting, exp_base_sl, path_sl, exp_field_sl)
        }
        il_ast::ExpKind::Call(_, _, args_sl) => lift_call_exp(ids_used, nesting, args_sl),
        il_ast::ExpKind::Iter(exp_inner_sl, iter_exp_sl) => {
            lift_iter_exp(ids_used, nesting, exp_inner_sl, iter_exp_sl)
        }
    }
}

/// Lifts the leftmost nested call from a sequence of expressions.
fn lift_exps(
    ids_used: &mut IdSet,
    nesting: CallNesting,
    exps_sl: &mut [sl::Exp],
) -> Option<LiftedCall> {
    // Search expressions in source order
    for exp_sl in exps_sl {
        if let Some(call_lifted) = lift_from_exp(ids_used, nesting, exp_sl) {
            return Some(call_lifted);
        }
    }
    None
}

// - Call extraction

/// Extracts a non-nullary call at the expression root.
fn try_lift_call(ids_used: &mut IdSet, exp_target_sl: &mut sl::Exp) -> Option<LiftedCall> {
    // Reject expressions that do not need a replacement binding
    let il_ast::ExpKind::Call(_, _, args_sl) = &exp_target_sl.node else {
        return None;
    };
    if args_sl.is_empty() {
        return None;
    }

    // Allocate the replacement variable from the call result type
    let typ_sl = crate::phrase! {
        node: exp_target_sl.note.as_ref().clone(),
        span: exp_target_sl.span.clone(),
    };
    let var_new_sl =
        il::fresh::var_from_typ(&IdMap::new(), ids_used, exp_target_sl.span.clone(), &typ_sl);
    ids_used.insert(var_new_sl.id.clone());
    let exp_new_sl = il::var::as_exp(true, &var_new_sl);
    let exp_call_sl = std::mem::replace(exp_target_sl, exp_new_sl.clone());
    let il_ast::ExpKind::Call(_, _, args_sl) = &exp_call_sl.node else {
        unreachable!();
    };

    // Track variables needed inside the call while crossing iterations
    let vars_call_sl = args_sl.as_slice().free_vars();
    Some(LiftedCall {
        exp_new_sl,
        exp_call_sl,
        vars_call_sl,
        var_new_sl,
        iter_exps_enclosing_sl: Vec::new(),
    })
}

// - Binary expression

fn lift_binary_exp(
    ids_used: &mut IdSet,
    nesting: CallNesting,
    exp_l_sl: &mut sl::Exp,
    exp_r_sl: &mut sl::Exp,
) -> Option<LiftedCall> {
    // Search the left operand first
    if let Some(call_lifted) = lift_from_exp(ids_used, nesting, exp_l_sl) {
        return Some(call_lifted);
    }

    // Search the right operand second
    lift_from_exp(ids_used, nesting, exp_r_sl)
}

// - Case expression

fn lift_case_exp(
    ids_used: &mut IdSet,
    nesting: CallNesting,
    not_exp_sl: &mut sl::NotExp,
) -> Option<LiftedCall> {
    let mut exps_sl = not_exp_sl.args().into_iter().cloned().collect::<Vec<_>>();
    let call_lifted = lift_exps(ids_used, nesting, &mut exps_sl)?;
    let mut exps_sl = exps_sl.into_iter();
    *not_exp_sl = not_exp_sl.map(|_| exps_sl.next().expect("lifting preserves notation arity"));
    Some(call_lifted)
}

// - Struct expression

fn lift_struct_exp(
    ids_used: &mut IdSet,
    nesting: CallNesting,
    fields_sl: &mut [il_ast::ExpField],
) -> Option<LiftedCall> {
    // Search field values in source order
    for (_, exp_sl) in fields_sl {
        if let Some(call_lifted) = lift_from_exp(ids_used, nesting, exp_sl) {
            return Some(call_lifted);
        }
    }
    None
}

// - Slice expression

fn lift_slice_exp(
    ids_used: &mut IdSet,
    nesting: CallNesting,
    exp_base_sl: &mut sl::Exp,
    exp_idx_sl: &mut sl::Exp,
    exp_len_sl: &mut sl::Exp,
) -> Option<LiftedCall> {
    // Search the base before slice operands
    if let Some(call_lifted) = lift_from_exp(ids_used, nesting, exp_base_sl) {
        return Some(call_lifted);
    }

    // Search the index before the length
    if let Some(call_lifted) = lift_from_exp(ids_used, nesting, exp_idx_sl) {
        return Some(call_lifted);
    }

    // Search the length last
    lift_from_exp(ids_used, nesting, exp_len_sl)
}

// - Update expression

fn lift_update_exp(
    ids_used: &mut IdSet,
    nesting: CallNesting,
    exp_base_sl: &mut sl::Exp,
    path_sl: &mut sl::Path,
    exp_field_sl: &mut sl::Exp,
) -> Option<LiftedCall> {
    // Search the base before the update path and field
    if let Some(call_lifted) = lift_from_exp(ids_used, nesting, exp_base_sl) {
        return Some(call_lifted);
    }

    // Search the path before the replacement field
    if let Some(call_lifted) = lift_path(ids_used, nesting, path_sl) {
        return Some(call_lifted);
    }

    // Search the replacement field last
    lift_from_exp(ids_used, nesting, exp_field_sl)
}

// - Call expression

fn lift_call_exp(
    ids_used: &mut IdSet,
    nesting: CallNesting,
    args_sl: &mut [sl::Arg],
) -> Option<LiftedCall> {
    // Search expression arguments in source order
    for arg_sl in args_sl {
        let il_ast::ArgKind::Exp(exp_sl) = &mut arg_sl.node else {
            continue;
        };
        if let Some(call_lifted) = lift_from_exp(ids_used, nesting, exp_sl) {
            return Some(call_lifted);
        }
    }
    None
}

// - Iterated expression

/// Lifts a call through an iteration while preserving its variable dimensions.
fn lift_iter_exp(
    ids_used: &mut IdSet,
    nesting: CallNesting,
    exp_inner_sl: &mut sl::Exp,
    iter_exp_sl: &mut sl::ExpIter,
) -> Option<LiftedCall> {
    let call_lifted = lift_from_exp(ids_used, nesting, exp_inner_sl)?;
    Some(lift_call_through_exp_iter(exp_inner_sl, iter_exp_sl, call_lifted))
}

fn lift_call_through_exp_iter(
    exp_inner_sl: &sl::Exp,
    (iter, vars_bound_sl): &mut sl::ExpIter,
    call_lifted: LiftedCall,
) -> LiftedCall {
    let mut vars_remaining_sl = exp_inner_sl.free_vars();
    lift_call_through_iter(&mut vars_remaining_sl, *iter, vars_bound_sl, call_lifted)
}

/// Propagates a lifted call through one enclosing iteration.
fn lift_call_through_iter(
    vars_remaining_sl: &mut [sl::Var],
    iter: sl::Iter,
    vars_bound_sl: &mut Vec<sl::Var>,
    mut call_lifted: LiftedCall,
) -> LiftedCall {
    // Find iteration variables used inside the lifted call
    let vars_call_matched_sl = call_lifted
        .vars_call_sl
        .iter()
        .filter(|var_call_sl| {
            vars_bound_sl
                .iter()
                .any(|var_bound_sl| var_bound_sl.syntax_eq(var_call_sl))
        })
        .cloned()
        .collect::<Vec<_>>();
    let vars_remaining_matched_sl = vars_remaining_sl
        .iter()
        .filter(|var_remaining_sl| {
            vars_bound_sl
                .iter()
                .any(|var_bound_sl| var_bound_sl.syntax_eq(var_remaining_sl))
        })
        .cloned()
        .collect::<Vec<_>>();

    // Carry remaining variables to the next enclosing iteration
    for var_remaining_sl in vars_remaining_sl {
        if vars_remaining_matched_sl
            .iter()
            .any(|var_matched_sl| var_matched_sl.syntax_eq(var_remaining_sl))
        {
            var_remaining_sl.iters.push(iter);
        }
    }
    if vars_call_matched_sl.is_empty() {
        return call_lifted;
    }

    // Extend matched call variables through this iteration
    for var_call_sl in &mut call_lifted.vars_call_sl {
        if vars_call_matched_sl
            .iter()
            .any(|var_matched_sl| var_matched_sl.syntax_eq(var_call_sl))
        {
            var_call_sl.iters.push(iter);
        }
    }

    // Partition bindings between the lifted call and remaining expression
    let vars_iter_sl = vars_bound_sl
        .iter()
        .filter(|var_bound_sl| {
            vars_call_matched_sl
                .iter()
                .any(|var_matched_sl| var_matched_sl.syntax_eq(var_bound_sl))
        })
        .cloned()
        .collect::<Vec<_>>();
    let vars_kept_sl = vars_bound_sl
        .iter()
        .filter(|var_bound_sl| {
            !vars_call_matched_sl
                .iter()
                .any(|var_matched_sl| var_matched_sl.syntax_eq(var_bound_sl))
                || vars_remaining_matched_sl
                    .iter()
                    .any(|var_matched_sl| var_matched_sl.syntax_eq(var_bound_sl))
        })
        .cloned()
        .collect::<Vec<_>>();

    // Record the iteration on the fresh variable and replacement binding
    let var_new_inner_sl = call_lifted.var_new_sl.clone();
    call_lifted.var_new_sl.iters.push(iter);
    call_lifted
        .iter_exps_enclosing_sl
        .push((iter, vars_iter_sl));
    *vars_bound_sl = std::iter::once(var_new_inner_sl)
        .chain(vars_kept_sl)
        .collect();
    call_lifted
}

// == Paths

fn lift_path(
    ids_used: &mut IdSet,
    nesting: CallNesting,
    path_sl: &mut sl::Path,
) -> Option<LiftedCall> {
    match &mut path_sl.node {
        il_ast::PathKind::Root => None,
        il_ast::PathKind::Idx(path_inner_sl, exp_idx_sl) => {
            if let Some(call_lifted) = lift_path(ids_used, nesting, path_inner_sl) {
                return Some(call_lifted);
            }
            lift_from_exp(ids_used, nesting, exp_idx_sl)
        }
        il_ast::PathKind::Slice(path_inner_sl, exp_idx_sl, exp_len_sl) => {
            lift_slice_path(ids_used, nesting, path_inner_sl, exp_idx_sl, exp_len_sl)
        }
        il_ast::PathKind::Dot(path_inner_sl, _) => lift_path(ids_used, nesting, path_inner_sl),
    }
}

fn lift_slice_path(
    ids_used: &mut IdSet,
    nesting: CallNesting,
    path_inner_sl: &mut sl::Path,
    exp_idx_sl: &mut sl::Exp,
    exp_len_sl: &mut sl::Exp,
) -> Option<LiftedCall> {
    // Search the inner path before slice operands
    if let Some(call_lifted) = lift_path(ids_used, nesting, path_inner_sl) {
        return Some(call_lifted);
    }

    // Search the index before the length
    if let Some(call_lifted) = lift_from_exp(ids_used, nesting, exp_idx_sl) {
        return Some(call_lifted);
    }

    // Search the length last
    lift_from_exp(ids_used, nesting, exp_len_sl)
}

// == Instructions

// - Instruction

fn lift_instr_call(
    ids_used: &mut IdSet,
    instr_sl: &mut sl::Instr,
) -> Result<Option<LiftedCall>, ProseError> {
    let span = instr_sl.span.clone();
    Ok(match &mut instr_sl.node {
        sl::InstrKind::Let(instr_sl) => lift_let_instr(ids_used, instr_sl),
        sl::InstrKind::Rule(instr_sl) => lift_rule_instr(ids_used, instr_sl, &span)?,
        sl::InstrKind::Hold(instr_sl) => lift_hold_instr(ids_used, instr_sl),
        sl::InstrKind::Result(instr_sl) => {
            lift_exps(ids_used, CallNesting::None, &mut instr_sl.exps)
        }
        sl::InstrKind::Return(instr_sl) => {
            lift_from_exp(ids_used, CallNesting::None, &mut instr_sl.exp)
        }
        sl::InstrKind::If(_)
        | sl::InstrKind::Case(_)
        | sl::InstrKind::Group(_)
        | sl::InstrKind::Debug(_) => None,
    })
}

// - Let instruction

/// Lifts the leftmost nested call from a let instruction's right-hand side.
fn lift_let_instr(ids_used: &mut IdSet, instr_sl: &mut sl::LetInstr) -> Option<LiftedCall> {
    let call_lifted = lift_from_exp(ids_used, CallNesting::None, &mut instr_sl.exp_r)?;
    let vars_remaining_sl = instr_sl.exp_r.free_vars();
    Some(lift_call_through_instr_iters(vars_remaining_sl, &mut instr_sl.iter_instrs, call_lifted))
}

// - Rule instruction

/// Lifts the leftmost eligible call from a rule instruction's inputs.
fn lift_rule_instr(
    ids_used: &mut IdSet,
    instr_sl: &mut sl::RuleInstr,
    span: &Span,
) -> Result<Option<LiftedCall>, ProseError> {
    // Separate relation inputs from result positions
    let exps_sl = instr_sl
        .not_exp
        .args()
        .into_iter()
        .cloned()
        .collect::<Vec<_>>();
    let (mut exps_input_sl, exps_output_sl) = input::split(&instr_sl.input_hint, exps_sl)
        .map_err(|error| ProseError::new(ProseErrorKind::Input(error), span.clone()))?;
    let call_lifted = lift_exps(ids_used, CallNesting::Outer, &mut exps_input_sl);

    // Restore notation only when an input changed
    let Some(call_lifted) = call_lifted else {
        return Ok(None);
    };
    let vars_remaining_sl = exps_input_sl.as_slice().free_vars();
    let call_lifted =
        lift_call_through_instr_iters(vars_remaining_sl, &mut instr_sl.iter_instrs, call_lifted);
    replace_rule_not_exp_args(instr_sl, exps_input_sl, exps_output_sl, span)?;
    Ok(Some(call_lifted))
}

fn replace_rule_not_exp_args(
    instr_sl: &mut sl::RuleInstr,
    exps_input_sl: Vec<sl::Exp>,
    exps_output_sl: Vec<sl::Exp>,
    span: &Span,
) -> Result<(), ProseError> {
    let exps_sl = input::combine(&instr_sl.input_hint, exps_input_sl, exps_output_sl)
        .map_err(|error| ProseError::new(ProseErrorKind::Input(error), span.clone()))?;
    let mut exps_sl = exps_sl.into_iter();
    instr_sl.not_exp = instr_sl
        .not_exp
        .map(|_| exps_sl.next().expect("lifting preserves notation arity"));
    Ok(())
}

// - Hold instruction

/// Lifts the leftmost eligible call from a hold instruction's arguments.
fn lift_hold_instr(ids_used: &mut IdSet, instr_sl: &mut sl::HoldInstr) -> Option<LiftedCall> {
    // Lift only calls owned by notation arguments
    let mut exps_sl = instr_sl
        .not_exp
        .args()
        .into_iter()
        .cloned()
        .collect::<Vec<_>>();
    let call_lifted = lift_exps(ids_used, CallNesting::Outer, &mut exps_sl);

    // Restore notation only when an argument changed
    let call_lifted = call_lifted?;
    let vars_remaining_sl = exps_sl.as_slice().free_vars();
    let call_lifted =
        lift_call_through_exp_iters(vars_remaining_sl, &mut instr_sl.iter_exps, call_lifted);
    let mut exps_sl = exps_sl.into_iter();
    instr_sl.not_exp = instr_sl
        .not_exp
        .map(|_| exps_sl.next().expect("lifting preserves notation arity"));
    Some(call_lifted)
}

// - Iteration bindings

fn lift_call_through_instr_iters(
    mut vars_remaining_sl: Vec<sl::Var>,
    iter_instrs_sl: &mut [sl::InstrIter],
    mut call_lifted: LiftedCall,
) -> LiftedCall {
    for iter_instr_sl in iter_instrs_sl {
        call_lifted = lift_call_through_iter(
            &mut vars_remaining_sl,
            iter_instr_sl.iter,
            &mut iter_instr_sl.vars_bound,
            call_lifted,
        );
    }
    call_lifted
}

fn lift_call_through_exp_iters(
    mut vars_remaining_sl: Vec<sl::Var>,
    iter_exps_sl: &mut [sl::ExpIter],
    mut call_lifted: LiftedCall,
) -> LiftedCall {
    for (iter, vars_bound_sl) in iter_exps_sl {
        call_lifted =
            lift_call_through_iter(&mut vars_remaining_sl, *iter, vars_bound_sl, call_lifted);
    }
    call_lifted
}

// == Entry point

/// Lifts all calls directly owned by an instruction.
///
/// The boolean reports whether the returned instruction contains new bindings.
pub(super) fn lift_instr(
    ids_used: &mut IdSet,
    mut instr_sl: sl::Instr,
) -> Result<(sl::Instr, bool), ProseError> {
    let mut calls_lifted = Vec::new();
    // Collect direct calls before nesting their bindings
    while let Some(call_lifted) = lift_instr_call(ids_used, &mut instr_sl)? {
        calls_lifted.push(call_lifted);
    }
    let lifted = !calls_lifted.is_empty();

    // Preserve left-to-right evaluation in the nesting order of let bindings
    for call_lifted in calls_lifted.into_iter().rev() {
        instr_sl = call_lifted.wrap_instr(instr_sl);
    }
    Ok((instr_sl, lifted))
}
