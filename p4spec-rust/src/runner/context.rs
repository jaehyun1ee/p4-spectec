//! Operation-local access to the components assembled by a runner.
//!
//! The context splits the runner into independent borrows for one evaluation.
//! An extern receives the same context and can reenter the interpreter after
//! its own shared borrow has been copied into a local reference.

use crate::{
    lang::{
        data::value::{Value, ValueArena},
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
    config: &'runner S::Config,
    interface: &'runner mut I,
    externs: &'runner E,
    arena: &'runner mut ValueArena,
}

impl<'runner, S, I, E> RunnerContext<'runner, S, I, E>
where
    S: Interpreter<I, E>,
    I: Interface,
    E: Extern,
{
    pub(super) fn new(
        spec: &'runner S::Spec,
        config: &'runner S::Config,
        interface: &'runner mut I,
        externs: &'runner E,
        arena: &'runner mut ValueArena,
    ) -> Self {
        Self {
            spec,
            config,
            interface,
            externs,
            arena,
        }
    }

    // - Semantic components

    pub fn spec(&self) -> &'runner S::Spec {
        self.spec
    }

    pub fn config(&self) -> &'runner S::Config {
        self.config
    }

    pub fn arena(&self) -> &ValueArena {
        self.arena
    }

    pub fn arena_mut(&mut self) -> &mut ValueArena {
        self.arena
    }

    // - Evaluation dispatch

    pub fn call_program(&mut self, name: &str, program: Value) -> Result<Vec<Value>, S::Error> {
        S::eval_program(self, name, program)
    }

    pub fn call_rel(&mut self, name: &str, values: &[Value]) -> Result<Vec<Value>, S::Error> {
        S::eval_rel(self, name, values)
    }

    pub fn call_func(
        &mut self,
        name: &str,
        targs: &[Typ],
        values: &[Value],
    ) -> Result<Value, S::Error> {
        S::eval_func(self, name, targs, values)
    }

    // - Host dispatch

    pub fn call_builtin(
        &mut self,
        id: &Id,
        targs: &[Typ],
        values: &[Value],
    ) -> Result<(Value, bool), S::Error> {
        let result = self.interface.call_builtin(self.arena, id, targs, values)?;
        Ok(result)
    }

    pub fn call_extern_rel(
        &mut self,
        name: &str,
        values: &[Value],
    ) -> Result<(Vec<Value>, bool), S::Error> {
        let externs = self.externs;
        externs.eval_rel(self, name, values)
    }

    pub fn call_extern_func(
        &mut self,
        name: &str,
        targs: &[Typ],
        values: &[Value],
    ) -> Result<(Value, bool), S::Error> {
        let externs = self.externs;
        externs.eval_func(self, name, targs, values)
    }
}
