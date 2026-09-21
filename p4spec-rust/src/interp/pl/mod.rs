//! Prose-language execution over annotated PL definitions

pub mod context;
mod eval;
pub mod flow;
mod prepare;

use crate::{
    interp::shared::{
        backtrack::Backtrack, cache::Cache, error::Error, eval::Invoker, prepare::ast as exec,
    },
    lang::{common::source::Span, data::value::Value, pl::ast},
    runner::{Extern, Interface, Interpreter, RunnerContext},
};

pub struct Config {
    pub(crate) cache: bool,
    pub(crate) det: bool,
    pub(crate) guard: bool,
}

impl Config {
    pub fn new(cache: bool, det: bool, guard: bool) -> Self {
        Self { cache, det, guard }
    }
}

pub struct PlInterp {
    pub(crate) config: Config,
    pub(crate) cache: Cache,
}

impl PlInterp {
    pub fn new(config: Config) -> Self {
        Self { config, cache: Cache::default() }
    }
}

impl<Iface: Interface, Ext: Extern> Invoker<Iface, Ext> for PlInterp {
    type Context<'global> = context::Context<'global>;

    fn invoke_func<'global>(
        runner_ctx: &mut RunnerContext<'_, Self, Iface, Ext>,
        ctx: &Self::Context<'global>,
        id: &exec::Id,
        targs: &[exec::Typ],
        values: &[Value],
    ) -> Backtrack<Value> {
        eval::invoke_func(runner_ctx, ctx, id, targs, values)
    }

    fn invoke_rel<'global>(
        runner_ctx: &mut RunnerContext<'_, Self, Iface, Ext>,
        ctx: &Self::Context<'global>,
        id: &exec::Id,
        values: &[Value],
    ) -> Backtrack<Vec<Value>> {
        eval::invoke_rel(runner_ctx, ctx, id, values)
    }
}

impl<Iface: Interface, Ext: Extern> Interpreter<Iface, Ext> for PlInterp {
    type Spec = context::Global;
    type Error = Error;

    fn clear(&mut self) {
        self.cache.clear();
    }
    fn reset(&mut self) {
        self.cache = Cache::default();
    }

    fn eval_program(
        runner_ctx: &mut RunnerContext<'_, Self, Iface, Ext>,
        name: &str,
        program: Value,
    ) -> Result<Vec<Value>, Error> {
        runner_ctx.call_rel(name, &[program])
    }

    fn eval_rel(
        runner_ctx: &mut RunnerContext<'_, Self, Iface, Ext>,
        name: &str,
        values: &[Value],
    ) -> Result<Vec<Value>, Error> {
        runner_ctx.interp_mut().cache.clear();
        let id = crate::phrase!(node: name.to_owned(), span: Span::default());
        let ctx = context::Context::new(runner_ctx.spec());
        if runner_ctx.interp().config.guard && !eval::cache_rel(runner_ctx, &ctx, &id) {
            eval::check_rel_inputs(runner_ctx.arena(), &ctx, &id, values).finish()?;
        }
        Self::invoke_rel(runner_ctx, &ctx, &id, values).finish()
    }

    fn eval_func(
        runner_ctx: &mut RunnerContext<'_, Self, Iface, Ext>,
        name: &str,
        targs: &[ast::Typ],
        values: &[Value],
    ) -> Result<Value, Error> {
        runner_ctx.interp_mut().cache.clear();
        let id = crate::phrase!(node: name.to_owned(), span: Span::default());
        let ctx = context::Context::new(runner_ctx.spec());
        if runner_ctx.interp().config.guard && !eval::cache_func(runner_ctx, &ctx, &id, values) {
            eval::check_func_inputs(runner_ctx.arena(), &ctx, &id, targs, values).finish()?;
        }
        Self::invoke_func(runner_ctx, &ctx, &id, targs, values).finish()
    }
}
