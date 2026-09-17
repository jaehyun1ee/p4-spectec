//! AL assignment, expression, argument, path, premise, and invocation evaluation

pub mod assign;
pub mod call;
pub mod expr;
pub mod iter;
pub mod prem;

use super::{AlInterp, context::Context};
use crate::{
    interp::shared::{backtrack::Backtrack, eval::Evaluator},
    lang::{al::ast, common::source::Span, data::value::Value},
    runner::{Extern, Interface, RunnerContext},
};

impl<'global, Iface: Interface, Exn: Extern> Evaluator<Context<'global>, Iface, Exn> for AlInterp {
    fn invoke_func(
        runner: &mut RunnerContext<'_, AlInterp, Iface, Exn>,
        ctx: &Context<'global>,
        id: &ast::Id,
        targs: &[ast::Typ],
        values: &[Value],
    ) -> Backtrack<Value> {
        call::invoke_func(runner, ctx, id, targs, values)
    }

    fn map_list(
        runner: &mut RunnerContext<'_, AlInterp, Iface, Exn>,
        ctx: &Context<'global>,
        span: &Span,
        vars: &[ast::Var],
        eval: impl FnMut(
            &mut RunnerContext<'_, AlInterp, Iface, Exn>,
            &Context<'global>,
        ) -> Backtrack<Value>,
    ) -> Backtrack<Vec<Value>> {
        iter::map_list(runner, ctx, span, vars, eval)
    }

    fn map_opt(
        runner: &mut RunnerContext<'_, AlInterp, Iface, Exn>,
        ctx: &Context<'global>,
        span: &Span,
        vars: &[ast::Var],
        eval: impl FnMut(
            &mut RunnerContext<'_, AlInterp, Iface, Exn>,
            &Context<'global>,
        ) -> Backtrack<Value>,
    ) -> Backtrack<Option<Value>> {
        iter::map_opt(runner, ctx, span, vars, eval)
    }
}
