//! Structured-language execution over the composed runner context

pub mod context;
pub mod flow;

pub mod eval;

use crate::interp::shared::{cache::Cache, error::Error, eval::Invoker};
use crate::{
    lang::{common::source::Span, data::value::Value, sl::ast},
    runner::{Extern, Interface, Interpreter, RunnerContext},
};
use context::{Context, Global};

/// Configuration for the SL interpreter
pub struct Config {
    cache: bool,
    det: bool,
    guard: bool,
}

impl Config {
    pub fn new(cache: bool, det: bool, guard: bool) -> Self {
        Self { cache, det, guard }
    }
}

pub struct SlInterp {
    config: Config,
    cache: Cache,
}

impl SlInterp {
    pub fn new(config: Config) -> Self {
        Self { config, cache: Cache::default() }
    }
}

impl<Iface: Interface, Ext: Extern> Interpreter<Iface, Ext> for SlInterp {
    type Spec = Global;
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
        let ctx = Context::new(runner_ctx.spec());
        if runner_ctx.interp().config.guard && !eval::call::cache_rel(runner_ctx, &ctx, &id) {
            eval::call::check_rel_inputs(runner_ctx.arena(), &ctx, &id, values)
                .guard()
                .finish()?;
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
        let ctx = Context::new(runner_ctx.spec());
        if runner_ctx.interp().config.guard
            && !eval::call::cache_func(runner_ctx, &ctx, &id, values)
        {
            eval::call::check_func_inputs(runner_ctx.arena(), &ctx, &id, targs, values)
                .guard()
                .finish()?;
        }
        Self::invoke_func(runner_ctx, &ctx, &id, targs, values).finish()
    }
}
