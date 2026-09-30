//! PL expression evaluation through the shared evaluator
//!
//! `eval_exp` and `eval_exps` use `strip` to remove prose hints,
//! then delegate to shared expression evaluation with the same slots.
//! Types and spans survive while the annotated PL tree stays intact.

use std::borrow::Borrow;

use crate::lang::data::value::Value;

use crate::runtime::envs::interp::pl::ast_prepared as ast;

use crate::runner::{Extern, Interface, RunnerContext};

use crate::interp::shared::{
    backtrack::{Backtrack, unwrap_from_result},
    eval::expr as shared,
};

use crate::interp::pl::{PlInterp, context::Context};

use super::strip::strip_exp;

// = Expression evaluation

/// Evaluates a PL expression after removing its prose hints.
pub(super) fn eval_exp<Iface: Interface, Ext: Extern>(
    runner_ctx: &mut RunnerContext<'_, PlInterp, Iface, Ext>,
    ctx: &Context<'_>,
    exp: &ast::Exp,
) -> Backtrack<Value> {
    let exp_shared =
        unwrap_from_result!(strip_exp(runner_ctx.arena_mut().shapes_mut(), exp), &exp.node.span);
    shared::eval_exp(runner_ctx, ctx, &exp_shared)
}

/// Evaluates PL expressions in order through the shared evaluator.
pub(super) fn eval_exps<Iface: Interface, Ext: Extern, T: Borrow<ast::Exp>>(
    runner_ctx: &mut RunnerContext<'_, PlInterp, Iface, Ext>,
    ctx: &Context<'_>,
    exps: &[T],
) -> Backtrack<Vec<Value>> {
    let mut exps_shared = Vec::with_capacity(exps.len());
    for exp in exps {
        let exp = exp.borrow();
        let exp_shared = unwrap_from_result!(
            strip_exp(runner_ctx.arena_mut().shapes_mut(), exp),
            &exp.node.span
        );
        exps_shared.push(exp_shared);
    }
    shared::eval_exps(runner_ctx, ctx, &exps_shared)
}
