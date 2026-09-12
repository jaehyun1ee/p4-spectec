//! Algorithmic-language execution over the composed runner

pub mod backtrack;
pub mod cache;
pub mod context;
pub mod error;

pub mod eval;
pub mod util;

use crate::{
    lang::{al::ast, common::source::Span, data::value::Value},
    runner::{Extern, Interface, Interpreter, RunnerContext},
};
use cache::Cache;
use context::{Context, Global};
use error::Error;

/// Configuration for the AL interpreter
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

pub struct AlInterp {
    config: Config,
    cache: Cache,
}

impl AlInterp {
    pub fn new(config: Config) -> Self {
        Self {
            config,
            cache: Cache::default(),
        }
    }
}

impl<Iface: Interface, Exn: Extern> Interpreter<Iface, Exn> for AlInterp {
    type Spec = Global;
    type Error = Error;

    fn clear(&mut self) {
        self.cache.clear();
    }

    fn reset(&mut self) {
        self.cache = Cache::default();
    }

    fn eval_program(
        runner: &mut RunnerContext<'_, Self, Iface, Exn>,
        name: &str,
        program: Value,
    ) -> Result<Vec<Value>, Error> {
        runner.call_rel(name, &[program])
    }

    fn eval_rel(
        runner: &mut RunnerContext<'_, Self, Iface, Exn>,
        name: &str,
        values: &[Value],
    ) -> Result<Vec<Value>, Error> {
        runner.interp_mut().cache.clear();
        let id = crate::phrase!(node: name.to_owned(), span: Span::default());
        let ctx = Context::new(runner.spec());
        if runner.interp().config.guard && !eval::call::cache_rel(runner, &ctx, &id) {
            eval::call::check_rel_inputs(runner.arena(), &ctx, &id, values)
                .guard()
                .finish()?;
        }
        eval::call::invoke_rel(runner, &ctx, &id, values).finish()
    }
    fn eval_func(
        runner: &mut RunnerContext<'_, Self, Iface, Exn>,
        name: &str,
        targs: &[ast::Typ],
        values: &[Value],
    ) -> Result<Value, Error> {
        runner.interp_mut().cache.clear();
        let id = crate::phrase!(node: name.to_owned(), span: Span::default());
        let ctx = Context::new(runner.spec());
        if runner.interp().config.guard && !eval::call::cache_func(runner, &ctx, &id, values) {
            eval::call::check_func_inputs(runner.arena(), &ctx, &id, targs, values)
                .guard()
                .finish()?;
        }
        eval::call::invoke_func(runner, &ctx, &id, targs, values).finish()
    }
}
