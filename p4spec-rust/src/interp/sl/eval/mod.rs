//! SL assignment, expression, instruction, and invocation evaluation

pub mod assign;
pub mod call;
pub mod expr;
pub mod instr;

use super::{SlInterp, context::Context};
use crate::{
    interp::shared::{backtrack::Backtrack, eval::Invoker},
    lang::{data::value::Value, sl::ast},
    runner::{Extern, Interface, RunnerContext},
};

impl<Iface: Interface, Ext: Extern> Invoker<Iface, Ext> for SlInterp {
    type Context<'global> = Context<'global>;

    fn invoke_func<'global>(
        runner_ctx: &mut RunnerContext<'_, SlInterp, Iface, Ext>,
        ctx: &Context<'global>,
        id: &ast::Id,
        targs: &[ast::Typ],
        values: &[Value],
    ) -> Backtrack<Value> {
        call::invoke_func(runner_ctx, ctx, id, targs, values)
    }

    fn invoke_rel<'global>(
        runner_ctx: &mut RunnerContext<'_, SlInterp, Iface, Ext>,
        ctx: &Context<'global>,
        id: &ast::Id,
        values: &[Value],
    ) -> Backtrack<Vec<Value>> {
        call::invoke_rel(runner_ctx, ctx, id, values)
    }
}
