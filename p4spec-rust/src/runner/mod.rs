//! Static assembly of the specification execution components.
//!
//! `Runner<S, I, E>` owns one interpreter stage, builtin interface, and extern
//! implementation. Each evaluation splits those components into a short-lived
//! context. For example, a function call may dispatch to an extern, which can
//! call another specification function through that same context before
//! returning its value and side-effect flag.

mod context;
mod externs;
mod interface;
mod interpreter;

use std::rc::Rc;

use crate::lang::{data::value::Value, il::ast::Typ};

pub use context::RunnerContext;
pub use externs::{Extern, ExternError, NullExtern};
pub use interface::{BuiltinInterface, Interface, InterfaceError, NullInterface};
pub use interpreter::Interpreter;

// == Runner assembly

/// An interpreter, builtin interface, and extern implementation assembled as
/// one stateful execution unit.
pub struct Runner<S, I, E>
where
    S: Interpreter<I, E>,
    I: Interface,
    E: Extern,
{
    spec: S::Spec,
    config: S::Config,
    interface: I,
    externs: E,
}

impl<S, I, E> Runner<S, I, E>
where
    S: Interpreter<I, E>,
    I: Interface,
    E: Extern,
{
    pub fn new(spec: S::Spec, config: S::Config, interface: I, externs: E) -> Self {
        Self {
            spec,
            config,
            interface,
            externs,
        }
    }

    /// Borrows the assembled components for a stage-specific evaluation entry
    pub fn context(&mut self) -> RunnerContext<'_, S, I, E> {
        RunnerContext::new(&self.spec, &self.config, &mut self.interface, &self.externs)
    }

    // - Evaluation

    pub fn eval_program(
        &mut self,
        name: &str,
        program: Rc<Value>,
    ) -> Result<Vec<Rc<Value>>, S::Error> {
        let mut context = self.context();
        context.call_program(name, program)
    }

    pub fn eval_rel(
        &mut self,
        name: &str,
        values: &[Rc<Value>],
    ) -> Result<Vec<Rc<Value>>, S::Error> {
        let mut context = self.context();
        context.call_rel(name, values)
    }

    pub fn eval_func(
        &mut self,
        name: &str,
        targs: &[Typ],
        values: &[Rc<Value>],
    ) -> Result<Rc<Value>, S::Error> {
        let mut context = self.context();
        context.call_func(name, targs, values)
    }

    // - Lifecycle

    /// Resets the builtin interface and extern implementation.
    pub fn clear(&mut self) {
        self.externs.clear();
        self.interface.clear();
    }
}
