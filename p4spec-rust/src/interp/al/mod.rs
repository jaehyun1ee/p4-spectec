//! Algorithmic-language execution over the composed runner

pub mod backtrack;
pub mod context;
pub mod error;
pub mod state;

pub mod eval;
pub mod util;

use crate::{
    lang::{al::ast, common::source::Span, data::value::Value},
    runner::{Extern, Interface, Interpreter, RunnerContext},
};
use context::{Context, Global};
use error::Error;

pub struct Al;

/// Configuration for the AL interpreter
pub struct Config {
    /// Eligible public calls bypass input guards when caching is enabled
    cache: bool,
    det: bool,
    guard: bool,
}

impl Config {
    pub fn new(cache: bool, det: bool, guard: bool) -> Self {
        Self { cache, det, guard }
    }
}

impl<I: Interface, E: Extern> Interpreter<I, E> for Al {
    type Spec = Global;
    type Config = Config;
    type State = state::State;
    type Error = Error;

    fn clear(state: &mut Self::State) {
        state.clear();
    }

    fn eval_program(
        runner: &mut RunnerContext<'_, Self, I, E>,
        name: &str,
        program: Value,
    ) -> Result<Vec<Value>, Error> {
        runner.call_rel(name, &[program])
    }

    fn eval_rel(
        runner: &mut RunnerContext<'_, Self, I, E>,
        name: &str,
        values: &[Value],
    ) -> Result<Vec<Value>, Error> {
        runner.state_mut().clear();
        let id = crate::phrase!(node: name.to_owned(), span: Span::default());
        let ctx = Context::new(runner.spec());
        if runner.config().guard && !eval::call::cache_rel(runner, &ctx, &id) {
            eval::call::check_rel_inputs(runner.arena(), &ctx, &id, values)
                .guard()
                .finish()?;
        }
        eval::call::invoke_rel(runner, &ctx, &id, values).finish()
    }
    fn eval_func(
        runner: &mut RunnerContext<'_, Self, I, E>,
        name: &str,
        targs: &[ast::Typ],
        values: &[Value],
    ) -> Result<Value, Error> {
        runner.state_mut().clear();
        let id = crate::phrase!(node: name.to_owned(), span: Span::default());
        let ctx = Context::new(runner.spec());
        if runner.config().guard && !eval::call::cache_func(runner, &ctx, &id, values) {
            eval::call::check_func_inputs(runner.arena(), &ctx, &id, targs, values)
                .guard()
                .finish()?;
        }
        eval::call::invoke_func(runner, &ctx, &id, targs, values).finish()
    }
}
