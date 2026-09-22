//! PL assignment, expression, instruction, and invocation evaluation
//!
//! `Invoker` for `PlInterp` delegates to `call`;
//! `instr` evaluates group and dispatch blocks into flows;
//! `assign` binds parameters; `expr` re-exports shared expression evaluation.

mod assign;
pub(super) mod call;
mod expr;
mod instr;

use crate::{
    interp::{
        pl::{PlInterp, context},
        shared::{backtrack::Backtrack, eval::Invoker, prepare::ast as exec},
    },
    lang::data::value::Value,
    runner::{Extern, Interface, RunnerContext},
};

impl<Iface: Interface, Ext: Extern> Invoker<Iface, Ext> for PlInterp {
    type Context<'global> = context::Context<'global>;

    fn invoke_func<'global>(
        runner_ctx: &mut RunnerContext<'_, Self, Iface, Ext>,
        ctx: &Self::Context<'global>,
        id: &exec::Id,
        targs: &[exec::Typ],
        values: &[Value],
    ) -> Backtrack<Value> {
        call::invoke_func(runner_ctx, ctx, id, targs, values)
    }

    fn invoke_rel<'global>(
        runner_ctx: &mut RunnerContext<'_, Self, Iface, Ext>,
        ctx: &Self::Context<'global>,
        id: &exec::Id,
        values: &[Value],
    ) -> Backtrack<Vec<Value>> {
        call::invoke_rel(runner_ctx, ctx, id, values)
    }
}
