//! AL assignment, expression, argument, path, premise, and invocation evaluation

pub mod assign;
pub mod call;
pub mod expr;
pub mod prem;

use super::{AlInterp, context::Context};
use crate::{
    interp::shared::{backtrack::Backtrack, eval::Invoker},
    lang::{al::ast, data::value::Value},
    runner::{Extern, Interface, RunnerContext},
};

impl<Iface: Interface, Exn: Extern> Invoker<Iface, Exn> for AlInterp {
    type Context<'global> = Context<'global>;

    fn invoke_func<'global>(
        runner_ctx: &mut RunnerContext<'_, AlInterp, Iface, Exn>,
        ctx: &Context<'global>,
        id: &ast::Id,
        targs: &[ast::Typ],
        values: &[Value],
    ) -> Backtrack<Value> {
        call::invoke_func(runner_ctx, ctx, id, targs, values)
    }

    fn invoke_rel<'global>(
        runner_ctx: &mut RunnerContext<'_, AlInterp, Iface, Exn>,
        ctx: &Context<'global>,
        id: &ast::Id,
        values: &[Value],
    ) -> Backtrack<Vec<Value>> {
        call::invoke_rel(runner_ctx, ctx, id, values)
    }
}
