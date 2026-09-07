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
}

impl Config {
    pub fn new(det: bool) -> Self {
        Self { det }
    }
}

impl<I: Interface, E: Extern> Interpreter<I, E> for Al {
    type Spec = Global;
    type Config = Config;
    type Error = Error;

    fn eval_program(
        runner: &mut RunnerContext<'_, Self, I, E>,
        name: &str,
        program: Rc<Value>,
    ) -> Result<Vec<Rc<Value>>, Error> {
        runner.call_rel(name, &[program])
    }

    fn eval_rel(
        runner: &mut RunnerContext<'_, Self, I, E>,
        name: &str,
        values: &[Rc<Value>],
    ) -> Result<Vec<Rc<Value>>, Error> {
        let id = crate::phrase!(node: name.to_owned(), span: Span::default());
        let ctx = Context::new(runner.spec());
        eval::call::invoke_rel(runner, &ctx, &id, values).finish()
    }
    fn eval_func(
        runner: &mut RunnerContext<'_, Self, I, E>,
        name: &str,
        targs: &[ast::Typ],
        values: &[Rc<Value>],
    ) -> Result<Rc<Value>, Error> {
        let id = crate::phrase!(node: name.to_owned(), span: Span::default());
        let ctx = Context::new(runner.spec());
        eval::call::invoke_func(runner, &ctx, &id, targs, values).finish()
    }
}
