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
    traits::{
        eq::SyntaxEq,
        free::{FreeIds, FreeVars},
    },
};

use super::super::{ProseError, ProseErrorKind};

// == Lifted calls

/// Controls whether a call at the expression root is eligible for lifting.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum RootCallPolicy {
    Preserve,
    Lift,
}

/// Describes one call removed from an expression and its replacement binding.
struct LiftedCall {
    exp_replacement_sl: sl::Exp,
    exp_call_sl: sl::Exp,
    vars_inner: Vec<sl::Var>,
    vars_outer: Vec<sl::Var>,
    var_fresh: sl::Var,
    iter_exps: Vec<sl::ExpIter>,
}

impl LiftedCall {
    /// Wraps an instruction in the let binding for this lifted call.
    fn wrap_instr(self, instr_sl: sl::Instr) -> sl::Instr {
        let Self { exp_replacement_sl, exp_call_sl, var_fresh, iter_exps, .. } = self;
        // Separate callee dimensions from iterations crossed during lifting
        let num_iters_enclosing = iter_exps.len();
        let num_iters_callee = var_fresh.iters.len() - num_iters_enclosing;
        let mut var_bind = sl::Var {
            id: var_fresh.id,
            typ: var_fresh.typ,
            iters: var_fresh.iters[..num_iters_callee].to_vec(),
        };

        // Rebuild instruction iterations from the inside out
        let mut iter_instrs = Vec::new();
        for (iter, vars_bound) in iter_exps {
            iter_instrs.push(sl::InstrIter { iter, vars_bound, vars_bind: vec![var_bind.clone()] });
            var_bind.iters.push(iter);
        }

        // Bind the extracted call immediately before its owning instruction
        let span = instr_sl.span.clone();
        crate::phrase! {
            node: sl::InstrKind::Let(sl::LetInstr {
                exp_l: exp_replacement_sl,
                exp_r: exp_call_sl,
                iter_instrs,
                block: vec![instr_sl],
            }),
            span: span,
        }
    }

    fn with_outer_vars(mut self, vars_outer: Vec<sl::Var>) -> Self {
        for var_outer in vars_outer {
            if !self.vars_outer.iter().any(|var| var.syntax_eq(&var_outer)) {
                self.vars_outer.push(var_outer);
            }
        }
        self
    }
}

// == Expressions

// - Expression

/// Lifts the leftmost eligible call from an expression.
fn lift_exp(
    ids_used: &mut IdSet,
    ids_iter_local: &IdSet,
    exp_target_sl: &mut sl::Exp,
    root_call_policy: RootCallPolicy,
) -> Option<LiftedCall> {
    // Try the expression root when the owning instruction permits it
    if root_call_policy == RootCallPolicy::Lift
        && let Some(call_lifted) = lift_root_call(ids_used, ids_iter_local, exp_target_sl)
    {
        return Some(call_lifted);
    }
    // Continue into children in source order
    lift_exp_kind(ids_used, ids_iter_local, &mut exp_target_sl.node, root_call_policy)
}

fn lift_exp_kind(
    ids_used: &mut IdSet,
    ids_iter_local: &IdSet,
    exp_kind_sl: &mut sl::ExpKind,
    root_call_policy: RootCallPolicy,
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
        | il_ast::ExpKind::Dot(exp_inner_sl, _) => {
            lift_exp(ids_used, ids_iter_local, exp_inner_sl, RootCallPolicy::Lift)
        }
        il_ast::ExpKind::Bin(_, _, exp_l_sl, exp_r_sl)
        | il_ast::ExpKind::Cmp(_, _, exp_l_sl, exp_r_sl)
        | il_ast::ExpKind::Cons(exp_l_sl, exp_r_sl)
        | il_ast::ExpKind::Cat(exp_l_sl, exp_r_sl)
        | il_ast::ExpKind::Mem(exp_l_sl, exp_r_sl)
        | il_ast::ExpKind::Idx(exp_l_sl, exp_r_sl) => {
            lift_binary_exp(ids_used, ids_iter_local, exp_l_sl, exp_r_sl)
        }
        il_ast::ExpKind::Tuple(exps_sl) | il_ast::ExpKind::List(exps_sl) => {
            lift_exps(ids_used, ids_iter_local, exps_sl, RootCallPolicy::Lift)
        }
        il_ast::ExpKind::Case(not_exp_sl) => lift_case_exp(ids_used, ids_iter_local, not_exp_sl),
        il_ast::ExpKind::Str(fields_sl) => lift_struct_exp(ids_used, ids_iter_local, fields_sl),
        il_ast::ExpKind::Opt(Some(exp_sl)) => {
            lift_exp(ids_used, ids_iter_local, exp_sl, RootCallPolicy::Lift)
        }
        il_ast::ExpKind::Opt(None) => None,
        il_ast::ExpKind::Slice(exp_base_sl, exp_idx_sl, exp_len_sl) => {
            lift_slice_exp(ids_used, ids_iter_local, exp_base_sl, exp_idx_sl, exp_len_sl)
        }
        il_ast::ExpKind::Upd(exp_base_sl, path_sl, exp_field_sl) => {
            lift_update_exp(ids_used, ids_iter_local, exp_base_sl, path_sl, exp_field_sl)
        }
        il_ast::ExpKind::Call(_, _, args_sl) => lift_call_exp(ids_used, ids_iter_local, args_sl),
        il_ast::ExpKind::Iter(exp_inner_sl, iter_exp_sl) => {
            lift_iter_exp(ids_used, ids_iter_local, exp_inner_sl, iter_exp_sl, root_call_policy)
        }
    }
}

// - Root call

/// Lifts a call at the expression root when its arguments cross no local iteration.
fn lift_root_call(
    ids_used: &mut IdSet,
    ids_iter_local: &IdSet,
    exp_target_sl: &mut sl::Exp,
) -> Option<LiftedCall> {
    // Reject non-calls and calls that cannot cross a local iteration
    let il_ast::ExpKind::Call(_, _, args_sl) = &exp_target_sl.node else {
        return None;
    };
    if args_sl.is_empty() || args_reference_local(ids_iter_local, args_sl) {
        return None;
    }

    // Allocate the replacement variable from the call result type
    let typ = crate::phrase! {
        node: exp_target_sl.note.as_ref().clone(),
        span: exp_target_sl.span.clone(),
    };
    let var_fresh =
        il::fresh::var_from_typ(&IdMap::new(), ids_used, exp_target_sl.span.clone(), &typ);
    ids_used.insert(var_fresh.id.clone());
    let exp_replacement_sl = il::var::as_exp(true, &var_fresh);
    let exp_call_sl = std::mem::replace(exp_target_sl, exp_replacement_sl.clone());
    let il_ast::ExpKind::Call(_, _, args_sl) = &exp_call_sl.node else {
        unreachable!();
    };

    // Track variables needed inside the call while crossing iterations
    let vars_inner = args_sl.as_slice().free_vars();
    Some(LiftedCall {
        exp_replacement_sl,
        exp_call_sl,
        vars_inner,
        vars_outer: Vec::new(),
        var_fresh,
        iter_exps: Vec::new(),
    })
}

fn args_reference_local(ids_iter_local: &IdSet, args_sl: &[sl::Arg]) -> bool {
    args_sl
        .free_ids()
        .iter()
        .any(|id| ids_iter_local.contains(id))
}

// - Binary expression

fn lift_binary_exp(
    ids_used: &mut IdSet,
    ids_iter_local: &IdSet,
    exp_l_sl: &mut sl::Exp,
    exp_r_sl: &mut sl::Exp,
) -> Option<LiftedCall> {
    // Search the left operand first
    if let Some(call_lifted) = lift_exp(ids_used, ids_iter_local, exp_l_sl, RootCallPolicy::Lift) {
        let vars_outer = exp_r_sl.free_vars();
        return Some(call_lifted.with_outer_vars(vars_outer));
    }

    // Keep left-operand variables when lifting from the right
    let call_lifted = lift_exp(ids_used, ids_iter_local, exp_r_sl, RootCallPolicy::Lift)?;
    let vars_outer = exp_l_sl.free_vars();
    Some(call_lifted.with_outer_vars(vars_outer))
}

// - Case expression

fn lift_case_exp(
    ids_used: &mut IdSet,
    ids_iter_local: &IdSet,
    not_exp_sl: &mut sl::NotExp,
) -> Option<LiftedCall> {
    let mut exps_sl = not_exp_sl.args().into_iter().cloned().collect::<Vec<_>>();
    let call_lifted = lift_exps(ids_used, ids_iter_local, &mut exps_sl, RootCallPolicy::Lift)?;
    replace_not_exp_args(not_exp_sl, exps_sl);
    Some(call_lifted)
}

fn replace_not_exp_args(not_exp_sl: &mut sl::NotExp, exps_sl: Vec<sl::Exp>) {
    let mut exps_sl = exps_sl.into_iter();
    *not_exp_sl = not_exp_sl.map(|_| exps_sl.next().expect("lifting preserves notation arity"));
}

// - Struct expression

fn lift_struct_exp(
    ids_used: &mut IdSet,
    ids_iter_local: &IdSet,
    fields_sl: &mut [il_ast::ExpField],
) -> Option<LiftedCall> {
    // Search field values in source order
    for idx in 0..fields_sl.len() {
        let (fields_before_sl, fields_rest_sl) = fields_sl.split_at_mut(idx);
        let ((_, exp_sl), fields_after_sl) = fields_rest_sl
            .split_first_mut()
            .expect("the index comes from the field slice");
        let Some(call_lifted) = lift_exp(ids_used, ids_iter_local, exp_sl, RootCallPolicy::Lift)
        else {
            continue;
        };

        // Preserve free variables from untouched fields
        let mut vars_outer = Vec::new();
        for (_, exp_other_sl) in fields_before_sl.iter().chain(fields_after_sl.iter()) {
            exp_other_sl.free_vars_into(&mut vars_outer);
        }
        return Some(call_lifted.with_outer_vars(vars_outer));
    }
    None
}

// - Slice expression

fn lift_slice_exp(
    ids_used: &mut IdSet,
    ids_iter_local: &IdSet,
    exp_base_sl: &mut sl::Exp,
    exp_idx_sl: &mut sl::Exp,
    exp_len_sl: &mut sl::Exp,
) -> Option<LiftedCall> {
    // Search the base before slice operands
    if let Some(call_lifted) = lift_exp(ids_used, ids_iter_local, exp_base_sl, RootCallPolicy::Lift)
    {
        let mut vars_outer = exp_idx_sl.free_vars();
        exp_len_sl.free_vars_into(&mut vars_outer);
        return Some(call_lifted.with_outer_vars(vars_outer));
    }

    // Search the index before the length
    if let Some(call_lifted) = lift_exp(ids_used, ids_iter_local, exp_idx_sl, RootCallPolicy::Lift)
    {
        let mut vars_outer = exp_base_sl.free_vars();
        exp_len_sl.free_vars_into(&mut vars_outer);
        return Some(call_lifted.with_outer_vars(vars_outer));
    }

    // Preserve base and index variables when lifting from the length
    let call_lifted = lift_exp(ids_used, ids_iter_local, exp_len_sl, RootCallPolicy::Lift)?;
    let mut vars_outer = exp_base_sl.free_vars();
    exp_idx_sl.free_vars_into(&mut vars_outer);
    Some(call_lifted.with_outer_vars(vars_outer))
}

// - Update expression

fn lift_update_exp(
    ids_used: &mut IdSet,
    ids_iter_local: &IdSet,
    exp_base_sl: &mut sl::Exp,
    path_sl: &mut sl::Path,
    exp_field_sl: &mut sl::Exp,
) -> Option<LiftedCall> {
    // Search the base before the update path and field
    if let Some(call_lifted) = lift_exp(ids_used, ids_iter_local, exp_base_sl, RootCallPolicy::Lift)
    {
        let mut vars_outer = path_sl.free_vars();
        exp_field_sl.free_vars_into(&mut vars_outer);
        return Some(call_lifted.with_outer_vars(vars_outer));
    }

    // Search the path before the replacement field
    if let Some(call_lifted) = lift_path(ids_used, ids_iter_local, path_sl) {
        let mut vars_outer = exp_base_sl.free_vars();
        exp_field_sl.free_vars_into(&mut vars_outer);
        return Some(call_lifted.with_outer_vars(vars_outer));
    }

    // Preserve base and path variables when lifting from the field
    let call_lifted = lift_exp(ids_used, ids_iter_local, exp_field_sl, RootCallPolicy::Lift)?;
    let mut vars_outer = exp_base_sl.free_vars();
    path_sl.free_vars_into(&mut vars_outer);
    Some(call_lifted.with_outer_vars(vars_outer))
}

// - Call expression

fn lift_call_exp(
    ids_used: &mut IdSet,
    ids_iter_local: &IdSet,
    args_sl: &mut [sl::Arg],
) -> Option<LiftedCall> {
    // Search expression arguments in source order
    for idx in 0..args_sl.len() {
        let (args_before_sl, args_rest_sl) = args_sl.split_at_mut(idx);
        let (arg_sl, args_after_sl) = args_rest_sl
            .split_first_mut()
            .expect("the index comes from the argument slice");
        let il_ast::ArgKind::Exp(exp_sl) = &mut arg_sl.node else {
            continue;
        };
        let Some(call_lifted) = lift_exp(ids_used, ids_iter_local, exp_sl, RootCallPolicy::Lift)
        else {
            continue;
        };

        // Preserve free variables from untouched arguments
        let mut vars_outer = args_before_sl.free_vars();
        args_after_sl.free_vars_into(&mut vars_outer);
        return Some(call_lifted.with_outer_vars(vars_outer));
    }
    None
}

// - Iterated expression

/// Lifts a call through an iteration while preserving its variable dimensions.
fn lift_iter_exp(
    ids_used: &mut IdSet,
    ids_iter_local: &IdSet,
    exp_inner_sl: &mut sl::Exp,
    (iter, vars_bound): &mut sl::ExpIter,
    root_call_policy: RootCallPolicy,
) -> Option<LiftedCall> {
    let mut call_lifted = lift_exp(ids_used, ids_iter_local, exp_inner_sl, root_call_policy)?;
    // Find iteration variables used inside the lifted call
    let vars_matched = call_lifted
        .vars_inner
        .iter()
        .filter(|var_inner| {
            vars_bound
                .iter()
                .any(|var_bound| var_bound.syntax_eq(var_inner))
        })
        .cloned()
        .collect::<Vec<_>>();
    if vars_matched.is_empty() {
        return Some(call_lifted);
    }

    // Extend matched call variables through this iteration
    for var_inner in &mut call_lifted.vars_inner {
        if vars_matched
            .iter()
            .any(|var_matched| var_matched.syntax_eq(var_inner))
        {
            var_inner.iters.push(*iter);
        }
    }

    // Partition bindings between the lifted call and remaining expression
    let vars_iter = vars_bound
        .iter()
        .filter(|var_bound| {
            vars_matched
                .iter()
                .any(|var_matched| var_matched.syntax_eq(var_bound))
        })
        .cloned()
        .collect::<Vec<_>>();
    let vars_kept = vars_bound
        .iter()
        .filter(|var_bound| {
            !vars_matched
                .iter()
                .any(|var_matched| var_matched.syntax_eq(var_bound))
                || call_lifted
                    .vars_outer
                    .iter()
                    .any(|var_outer| var_outer.syntax_eq(var_bound))
        })
        .cloned()
        .collect::<Vec<_>>();

    // Record the iteration on the fresh variable and replacement binding
    let var_fresh_inner = call_lifted.var_fresh.clone();
    call_lifted.var_fresh.iters.push(*iter);
    call_lifted.iter_exps.push((*iter, vars_iter));
    *vars_bound = std::iter::once(var_fresh_inner).chain(vars_kept).collect();
    Some(call_lifted)
}

// - Expression list

/// Lifts the leftmost eligible call from a sequence of expressions.
fn lift_exps(
    ids_used: &mut IdSet,
    ids_iter_local: &IdSet,
    exps_sl: &mut [sl::Exp],
    root_call_policy: RootCallPolicy,
) -> Option<LiftedCall> {
    // Search expressions in source order
    for idx in 0..exps_sl.len() {
        let (exps_before_sl, exps_rest_sl) = exps_sl.split_at_mut(idx);
        let (exp_sl, exps_after_sl) = exps_rest_sl
            .split_first_mut()
            .expect("the index comes from the expression slice");
        let Some(call_lifted) = lift_exp(ids_used, ids_iter_local, exp_sl, root_call_policy) else {
            continue;
        };

        // Preserve free variables from untouched siblings
        let mut vars_outer = exps_before_sl.free_vars();
        exps_after_sl.free_vars_into(&mut vars_outer);
        return Some(call_lifted.with_outer_vars(vars_outer));
    }
    None
}

// == Paths

fn lift_path(
    ids_used: &mut IdSet,
    ids_iter_local: &IdSet,
    path_sl: &mut sl::Path,
) -> Option<LiftedCall> {
    match &mut path_sl.node {
        il_ast::PathKind::Root => None,
        il_ast::PathKind::Idx(path_inner_sl, exp_idx_sl) => {
            if let Some(call_lifted) = lift_path(ids_used, ids_iter_local, path_inner_sl) {
                let vars_outer = exp_idx_sl.free_vars();
                return Some(call_lifted.with_outer_vars(vars_outer));
            }
            let call_lifted = lift_exp(ids_used, ids_iter_local, exp_idx_sl, RootCallPolicy::Lift)?;
            let vars_outer = path_inner_sl.free_vars();
            Some(call_lifted.with_outer_vars(vars_outer))
        }
        il_ast::PathKind::Slice(path_inner_sl, exp_idx_sl, exp_len_sl) => {
            lift_slice_path(ids_used, ids_iter_local, path_inner_sl, exp_idx_sl, exp_len_sl)
        }
        il_ast::PathKind::Dot(path_inner_sl, _) => {
            lift_path(ids_used, ids_iter_local, path_inner_sl)
        }
    }
}

fn lift_slice_path(
    ids_used: &mut IdSet,
    ids_iter_local: &IdSet,
    path_inner_sl: &mut sl::Path,
    exp_idx_sl: &mut sl::Exp,
    exp_len_sl: &mut sl::Exp,
) -> Option<LiftedCall> {
    // Search the inner path before slice operands
    if let Some(call_lifted) = lift_path(ids_used, ids_iter_local, path_inner_sl) {
        let mut vars_outer = exp_idx_sl.free_vars();
        exp_len_sl.free_vars_into(&mut vars_outer);
        return Some(call_lifted.with_outer_vars(vars_outer));
    }

    // Search the index before the length
    if let Some(call_lifted) = lift_exp(ids_used, ids_iter_local, exp_idx_sl, RootCallPolicy::Lift)
    {
        let mut vars_outer = path_inner_sl.free_vars();
        exp_len_sl.free_vars_into(&mut vars_outer);
        return Some(call_lifted.with_outer_vars(vars_outer));
    }

    // Preserve path and index variables when lifting from the length
    let call_lifted = lift_exp(ids_used, ids_iter_local, exp_len_sl, RootCallPolicy::Lift)?;
    let mut vars_outer = path_inner_sl.free_vars();
    exp_idx_sl.free_vars_into(&mut vars_outer);
    Some(call_lifted.with_outer_vars(vars_outer))
}

// == Instructions

// - Instruction

fn lift_instr_call(
    ids_used: &mut IdSet,
    instr_sl: &mut sl::Instr,
) -> Result<Option<LiftedCall>, ProseError> {
    let span = instr_sl.span.clone();
    Ok(match &mut instr_sl.node {
        sl::InstrKind::Let(instr_sl) => lift_exp(
            ids_used,
            &ids_bound_by_instr_iters(&instr_sl.iter_instrs),
            &mut instr_sl.exp_r,
            RootCallPolicy::Preserve,
        ),
        sl::InstrKind::Rule(instr_sl) => lift_rule_instr(ids_used, instr_sl, &span)?,
        sl::InstrKind::Hold(instr_sl) => lift_hold_instr(ids_used, instr_sl),
        sl::InstrKind::Result(instr_sl) => {
            lift_exps(ids_used, &IdSet::new(), &mut instr_sl.exps, RootCallPolicy::Preserve)
        }
        sl::InstrKind::Return(instr_sl) => {
            lift_exp(ids_used, &IdSet::new(), &mut instr_sl.exp, RootCallPolicy::Preserve)
        }
        sl::InstrKind::If(_)
        | sl::InstrKind::Case(_)
        | sl::InstrKind::Group(_)
        | sl::InstrKind::Debug(_) => None,
    })
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
    let call_lifted = lift_exps(
        ids_used,
        &ids_bound_by_instr_iters(&instr_sl.iter_instrs),
        &mut exps_input_sl,
        RootCallPolicy::Lift,
    );

    // Restore notation only when an input changed
    if call_lifted.is_some() {
        replace_rule_not_exp_args(instr_sl, exps_input_sl, exps_output_sl, span)?;
    }
    Ok(call_lifted)
}

fn replace_rule_not_exp_args(
    instr_sl: &mut sl::RuleInstr,
    exps_input_sl: Vec<sl::Exp>,
    exps_output_sl: Vec<sl::Exp>,
    span: &Span,
) -> Result<(), ProseError> {
    let exps_sl = input::combine(&instr_sl.input_hint, exps_input_sl, exps_output_sl)
        .map_err(|error| ProseError::new(ProseErrorKind::Input(error), span.clone()))?;
    replace_not_exp_args(&mut instr_sl.not_exp, exps_sl);
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
    let call_lifted = lift_exps(
        ids_used,
        &ids_bound_by_exp_iters(&instr_sl.iter_exps),
        &mut exps_sl,
        RootCallPolicy::Lift,
    );

    // Restore notation only when an argument changed
    if call_lifted.is_some() {
        replace_not_exp_args(&mut instr_sl.not_exp, exps_sl);
    }
    call_lifted
}

// - Iteration bindings

/// Collects identifiers bound by instruction iterations.
fn ids_bound_by_instr_iters(iter_instrs: &[sl::InstrIter]) -> IdSet {
    iter_instrs
        .iter()
        .flat_map(|iter_instr| iter_instr.vars_bound.iter())
        .map(|var| var.id.clone())
        .collect()
}

/// Collects identifiers bound by expression iterations.
fn ids_bound_by_exp_iters(iter_exps: &[sl::ExpIter]) -> IdSet {
    iter_exps
        .iter()
        .flat_map(|(_, vars)| vars)
        .map(|var| var.id.clone())
        .collect()
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
    let was_lifted = !calls_lifted.is_empty();

    // Preserve left-to-right evaluation in the nesting order of let bindings
    for call_lifted in calls_lifted.into_iter().rev() {
        instr_sl = call_lifted.wrap_instr(instr_sl);
    }
    Ok((instr_sl, was_lifted))
}
