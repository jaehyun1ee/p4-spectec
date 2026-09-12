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

pub struct RunnerContext<'runner, Interp, Iface, Exn>
where
    Interp: Interpreter<Iface, Exn>,
    Iface: Interface,
    Exn: Extern,
{
    arena: &'runner mut ValueArena,
    spec: &'runner Interp::Spec,
    interp: &'runner mut Interp,
    interface: &'runner mut Iface,
    external: &'runner Exn,
}

impl<'runner, Interp, Iface, Exn> RunnerContext<'runner, Interp, Iface, Exn>
where
    Interp: Interpreter<Iface, Exn>,
    Iface: Interface,
    Exn: Extern,
{
    pub(super) fn new(
        arena: &'runner mut ValueArena,
        spec: &'runner Interp::Spec,
        interp: &'runner mut Interp,
        interface: &'runner mut Iface,
        external: &'runner Exn,
    ) -> Self {
        Self {
            arena,
            spec,
            interp,
            interface,
            external,
        }
    }

    // - Semantic components

    pub fn spec(&self) -> &'runner Interp::Spec {
        self.spec
    }

    pub fn interp(&self) -> &Interp {
        self.interp
    }

    pub fn interp_mut(&mut self) -> &mut Interp {
        self.interp
    }

    pub fn arena(&self) -> &ValueArena {
        self.arena
    }

    pub fn arena_mut(&mut self) -> &mut ValueArena {
        self.arena
    }

    // - Evaluation dispatch

    pub fn call_program(
        &mut self,
        name: &str,
        program: Value,
    ) -> Result<Vec<Value>, Interp::Error> {
        Interp::eval_program(self, name, program)
    }

    pub fn call_rel(&mut self, name: &str, values: &[Value]) -> Result<Vec<Value>, Interp::Error> {
        Interp::eval_rel(self, name, values)
    }

    pub fn call_func(
        &mut self,
        name: &str,
        targs: &[Typ],
        values: &[Value],
    ) -> Result<Value, Interp::Error> {
        Interp::eval_func(self, name, targs, values)
    }

    // - Host dispatch

    pub fn call_builtin(
        &mut self,
        id: &Id,
        targs: &[Typ],
        values: &[Value],
    ) -> Result<(Value, bool), Interp::Error> {
        let result = self.interface.call_builtin(self.arena, id, targs, values)?;
        Ok(result)
    }

    pub fn call_extern_rel(
        &mut self,
        name: &str,
        values: &[Value],
    ) -> Result<(Vec<Value>, bool), Interp::Error> {
        let external = self.external;
        external.eval_rel(self, name, values)
    }

    pub fn call_extern_func(
        &mut self,
        name: &str,
        targs: &[Typ],
        values: &[Value],
    ) -> Result<(Value, bool), Interp::Error> {
        let external = self.external;
        external.eval_func(self, name, targs, values)
    }
}
