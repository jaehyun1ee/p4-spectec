//! Structured-language execution over the composed runner

pub mod context;
pub use crate::interp::al::error;

pub mod expression;
pub mod instruction;
pub mod interpreter;

use crate::interp::al::cache::Cache;
use crate::{
    lang::{common::source::Span, data::value::Value, sl::ast},
    runner::{Extern, Interface, Interpreter, RunnerContext},
};
use context::{Context, Global};
use error::Error;

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
        Self {
            config,
            cache: Cache::default(),
        }
    }
}

impl<Iface: Interface, Exn: Extern> Interpreter<Iface, Exn> for SlInterp {
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
        interpreter::invoke_rel_entry(runner, &ctx, &id, values).finish()
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
        interpreter::invoke_func_entry(runner, &ctx, &id, targs, values).finish()
    }
}
