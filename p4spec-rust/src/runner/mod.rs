//! Static assembly of the specification execution components.
//!
//! `Runner<S, I, E>` owns one interpreter stage, builtin interface, and extern
//! implementation. Each evaluation splits those components into a short-lived
//! context, which also supplies the explicit path for extern reentry.

mod context;
mod externs;
mod interface;
mod interpreter;

use std::rc::Rc;

use crate::lang::{data::value::Value, il::ast::Typ};

pub use context::RunnerContext;
pub use externs::{Extern, ExternError, ExternErrorKind, NullExtern};
pub use interface::{
    BuiltinInterface, Interface, InterfaceError, InterfaceErrorKind, NullInterface,
};
pub use interpreter::Interpreter;

// == Runner assembly

pub struct Runner<S, I, E>
where
    S: Interpreter<I, E>,
    I: Interface,
    E: Extern,
{
    spec: S::Spec,
    interp_state: S::State,
    interface: I,
    externs: E,
}

impl<S, I, E> Runner<S, I, E>
where
    S: Interpreter<I, E>,
    I: Interface,
    E: Extern,
{
    pub fn new(spec: S::Spec, interp_state: S::State, interface: I, externs: E) -> Self {
        Self {
            spec,
            interp_state,
            interface,
            externs,
        }
    }

    // - Evaluation

    pub fn eval_rel(
        &mut self,
        name: &str,
        values: &[Rc<Value>],
    ) -> Result<Vec<Rc<Value>>, S::Error> {
        let mut context: RunnerContext<'_, S, I, E> = RunnerContext::new(
            &self.spec,
            &mut self.interp_state,
            &mut self.interface,
            &self.externs,
        );
        context.call_rel(name, values)
    }

    pub fn eval_func(
        &mut self,
        name: &str,
        targs: &[Typ],
        values: &[Rc<Value>],
    ) -> Result<Rc<Value>, S::Error> {
        let mut context: RunnerContext<'_, S, I, E> = RunnerContext::new(
            &self.spec,
            &mut self.interp_state,
            &mut self.interface,
            &self.externs,
        );
        context.call_func(name, targs, values)
    }
}
