//! Executes prepared constructor programs over list rows
//!
//! Row inputs replace private frame writes. Each constructor still allocates
//! through the common interner in its original evaluation order.

use smallvec::SmallVec;

use crate::lang::{
    common::source::Span,
    data::value::{Arena, Value, make},
    traits::print::Print,
};

use crate::runner::InterpreterError;

use crate::interp::shared::{
    backtrack::{Backtrack, from_result, ok, unwrap, unwrap_from_result},
    context::{IterContext, ReadContext},
    prepare::{
        ast,
        construct::{ConstructOp, ConstructPlan},
    },
    util::iterate_vars,
};

/// Evaluates a registered list map, retaining ordinary input and output checks.
pub(crate) fn map(
    arena: &mut Arena,
    ctx: &impl IterContext,
    exp: &ast::Exp,
    plan: &ConstructPlan,
) -> Backtrack<Value> {
    let ast::ExpKind::Iter(exp_inner, exp_iter) = &exp.node else {
        unreachable!("only a list map has a constructor plan");
    };
    let vars = &exp_iter.vars;
    let vars_outer = iterate_vars(ctx, vars, exp_iter.iter);
    let values_by_var =
        unwrap_from_result!(ctx.find_list_values_by_var(arena, &vars_outer), &exp.span);
    // Snapshot inputs before any constructor can grow arena storage
    let values_by_var: Vec<_> = values_by_var.into_iter().map(<[Value]>::to_vec).collect();
    let len = values_by_var.first().map_or(0, Vec::len);
    let mut values = Vec::with_capacity(len);
    let mut stack = SmallVec::<[Value; 16]>::new();
    for idx in 0..len {
        // Every ordinary input write checks its slot before evaluating the body
        for var in vars {
            let _ = ctx.find_value_at_slot(var.slot);
        }
        values.push(unwrap!(eval_row(
            arena,
            ctx,
            exp_inner,
            plan,
            &values_by_var,
            idx,
            &mut stack
        )));
    }
    ok!(unwrap_from_result!(
        make::list(arena, exp.note.clone(), values, Span::default()),
        &exp.span
    ))
}

/// Evaluates reads and constructors with one operand stack reused across rows.
fn eval_row(
    arena: &mut Arena,
    ctx: &impl ReadContext,
    exp: &ast::Exp,
    plan: &ConstructPlan,
    values_by_var: &[Vec<Value>],
    idx_row: usize,
    stack: &mut SmallVec<[Value; 16]>,
) -> Backtrack<Value> {
    stack.clear();
    for (idx, instr) in plan.instrs.iter().enumerate() {
        let len = match &instr.op {
            ConstructOp::Tuple(len) | ConstructOp::Case(_, len) | ConstructOp::List(len) => *len,
            ConstructOp::Struct(atoms) => atoms.len(),
            ConstructOp::Opt(present) => usize::from(*present),
            _ => 0,
        };
        let start = stack.len() - len;
        let values = &stack[start..];
        let value = match &instr.op {
            ConstructOp::Slot(slot) => {
                Ok(*ctx.find_value_at_slot(*slot).expect("value must be bound"))
            }
            ConstructOp::Column(idx) => Ok(values_by_var[*idx][idx_row]),
            ConstructOp::Bool(value) => make::bool(arena, *value, Span::default()),
            ConstructOp::Num(num) => make::num_ref(arena, num, Span::default()),
            ConstructOp::Text(text) => make::text_ref(arena, text, Span::default()),
            ConstructOp::Tuple(_) => {
                make::tuple_from_slice(arena, instr.typ.clone(), values, Span::default())
            }
            ConstructOp::Case(mixop, _) => {
                make::case_from_slice(arena, instr.typ.clone(), mixop, values, Span::default())
            }
            ConstructOp::Struct(atoms) => {
                let value_fields = atoms.iter().cloned().zip(values.iter().copied()).collect();
                make::structure(arena, instr.typ.clone(), value_fields, Span::default())
            }
            ConstructOp::Opt(_) => {
                make::opt(arena, instr.typ.clone(), values.first().copied(), Span::default())
            }
            ConstructOp::List(_) => {
                make::list_from_slice(arena, instr.typ.clone(), values, Span::default())
            }
        };
        let value = match from_result(value, &instr.span) {
            Ok(value) => value,
            Err(error) => return Err(with_frames(error, exp, plan, idx)),
        };
        stack.truncate(start);
        stack.push(value);
    }
    ok!(*stack
        .last()
        .expect("a constructor program produces one value"))
}

/// Reconstructs the original leaf-to-root expression trace only on failure.
#[cold]
fn with_frames(
    mut error: InterpreterError,
    exp: &ast::Exp,
    plan: &ConstructPlan,
    idx: usize,
) -> InterpreterError {
    let mut path = SmallVec::<[usize; 8]>::new();
    let mut idx = idx;
    while let Some(parent) = plan.instrs[idx].parent {
        path.push(plan.instrs[idx].child);
        idx = parent;
    }
    let mut exps = SmallVec::<[&ast::Exp; 8]>::new();
    exps.push(exp);
    for idx in path.into_iter().rev() {
        let exp = exps.last().expect("the root expression is present");
        let exp_child = match &exp.node {
            ast::ExpKind::Tuple(exps) | ast::ExpKind::List(exps) => &exps[idx],
            ast::ExpKind::Case(not_exp) => &not_exp.args()[idx],
            ast::ExpKind::Str(exp_fields) => &exp_fields[idx].exp,
            ast::ExpKind::Opt(Some(exp)) => exp,
            _ => unreachable!("a prepared parent has constructor children"),
        };
        exps.push(exp_child);
    }
    for exp in exps.into_iter().rev() {
        error = error
            .with_frame(exp.span, format!("while evaluating expression {}", Print::to_string(exp)));
    }
    error
}
