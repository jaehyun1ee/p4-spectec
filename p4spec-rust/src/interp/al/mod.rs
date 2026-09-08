//! Algorithmic-language execution over the composed runner

pub mod backtrack;
pub mod context;
pub mod error;

pub mod eval;
pub mod util;

use crate::{
    lang::{al::ast, common::source::Span, data::value::Value},
    runner::{Extern, Interface, Interpreter, RunnerContext},
};
use context::{Context, Global};
use error::Error;
use std::rc::Rc;

pub struct Al;

/// Configuration for the AL interpreter
pub struct Config {
    det: bool,
    guard: bool,
}

impl Config {
    pub fn new(det: bool, guard: bool) -> Self {
        Self { det, guard }
    }
}

impl<I: Interface, E: Extern> Interpreter<I, E> for Al {
    type Spec = Rc<Global>;
    type Config = Config;
    type Error = Error;

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
        let id = crate::phrase!(node: name.to_owned(), span: Span::default());
        let ctx = Context::new(runner.spec());
        if runner.config().guard {
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
        let id = crate::phrase!(node: name.to_owned(), span: Span::default());
        let ctx = Context::new(runner.spec());
        if runner.config().guard {
            eval::call::check_func_inputs(runner.arena(), &ctx, &id, targs, values)
                .guard()
                .finish()?;
        }
        eval::call::invoke_func(runner, &ctx, &id, targs, values).finish()
    }
}
