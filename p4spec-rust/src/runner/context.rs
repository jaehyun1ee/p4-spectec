//! Operation-local access to the components assembled by a runner.
//!
//! The context splits the runner into independent borrows for one evaluation.
//! An extern receives the same context and can reenter the interpreter after
//! its own shared borrow has been copied into a local reference.

use std::rc::Rc;

use crate::{
    lang::{
        data::value::Value,
        il::ast::{Id, Typ},
    },
    runner::{Extern, Interface, Interpreter},
};

// == Runner context

pub struct RunnerContext<'runner, S, I, E>
where
    S: Interpreter<I, E>,
    I: Interface,
    E: Extern,
{
    spec: &'runner S::Spec,
    interp_state: &'runner mut S::State,
    interface: &'runner mut I,
    externs: &'runner E,
}

impl<'runner, S, I, E> RunnerContext<'runner, S, I, E>
where
    S: Interpreter<I, E>,
    I: Interface,
    E: Extern,
{
    pub(super) fn new(
        spec: &'runner S::Spec,
        interp_state: &'runner mut S::State,
        interface: &'runner mut I,
        externs: &'runner E,
    ) -> Self {
        Self {
            spec,
            interp_state,
            interface,
            externs,
        }
    }

    // - Semantic components

    pub fn spec(&self) -> &S::Spec {
        self.spec
    }

    pub fn interp_state(&mut self) -> &mut S::State {
        self.interp_state
    }

    // - Evaluation dispatch

    pub fn call_rel(
        &mut self,
        name: &str,
        values: &[Rc<Value>],
    ) -> Result<Vec<Rc<Value>>, S::Error> {
        S::eval_rel(self, name, values)
    }

    pub fn call_func(
        &mut self,
        name: &str,
        targs: &[Typ],
        values: &[Rc<Value>],
    ) -> Result<Rc<Value>, S::Error> {
        S::eval_func(self, name, targs, values)
    }

    // - Host dispatch

    pub fn call_builtin(
        &mut self,
        id: &Id,
        targs: &[Typ],
        values: &[Rc<Value>],
    ) -> Result<(Rc<Value>, bool), S::Error> {
        let result = self.interface.call_builtin(id, targs, values)?;
        Ok(result)
    }

    pub fn call_extern_rel(
        &mut self,
        name: &str,
        values: &[Rc<Value>],
    ) -> Result<(Vec<Rc<Value>>, bool), S::Error> {
        let externs = self.externs;
        externs.eval_rel(self, name, values)
    }

    pub fn call_extern_func(
        &mut self,
        name: &str,
        targs: &[Typ],
        values: &[Rc<Value>],
    ) -> Result<(Rc<Value>, bool), S::Error> {
        let externs = self.externs;
        externs.eval_func(self, name, targs, values)
    }
}
