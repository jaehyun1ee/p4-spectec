//! Algorithmic-language execution over the composed runner

pub mod backtrack;
pub mod context;
pub mod error;

pub mod eval;
pub mod util;

use super::common::{Event, Observer};
use crate::{
    lang::{al::ast, common::source::Span, data::value::Value},
    runner::{Extern, Interface, Interpreter, RunnerContext},
};
use backtrack::Backtrack;
use context::{Context, Global};
use error::Error;
use std::rc::Rc;

pub struct Al;

pub struct State {
    det: bool,
    observer: Option<Box<dyn Observer>>,
}

impl State {
    pub fn new(det: bool) -> Self {
        Self {
            det,
            observer: None,
        }
    }
    pub fn set_observer(&mut self, observer: Option<Box<dyn Observer>>) {
        self.observer = observer;
    }
    pub(super) fn emit(&mut self, event: Event) {
        if let Some(observer) = &mut self.observer {
            observer.event(&event);
        }
    }
}

/// Evaluates an already parsed program through the selected relation
pub fn eval_program<I: Interface, E: Extern>(
    runner: &mut RunnerContext<'_, Al, I, E>,
    name: &str,
    program: Rc<Value>,
) -> Result<Vec<Rc<Value>>, Error> {
    runner
        .interp_state()
        .emit(Event::Program(Rc::clone(&program)));
    runner.call_rel(name, &[program])
}

fn finish<T>(result: Backtrack<T>) -> Result<T, Error> {
    match result {
        Backtrack::Ok(value) => Ok(value),
        Backtrack::Nondet(never, _) => match never {},
        Backtrack::Err(traces) | Backtrack::Unmatch(traces) => Err(Error::execution(traces)),
    }
}

impl<I: Interface, E: Extern> Interpreter<I, E> for Al {
    type Spec = Global;
    type State = State;
    type Error = Error;

    fn eval_rel(
        runner: &mut RunnerContext<'_, Self, I, E>,
        name: &str,
        values: &[Rc<Value>],
    ) -> Result<Vec<Rc<Value>>, Error> {
        let id = crate::phrase!(node: name.to_owned(), span: Span::default());
        let ctx = Context::new(runner.spec());
        finish(eval::call::invoke_rel(runner, &ctx, &id, values))
    }
    fn eval_func(
        runner: &mut RunnerContext<'_, Self, I, E>,
        name: &str,
        targs: &[ast::Typ],
        values: &[Rc<Value>],
    ) -> Result<Rc<Value>, Error> {
        let id = crate::phrase!(node: name.to_owned(), span: Span::default());
        let ctx = Context::new(runner.spec());
        finish(eval::call::invoke_func(runner, &ctx, &id, targs, values))
    }
    fn clear(_state: &mut State) {}
}
