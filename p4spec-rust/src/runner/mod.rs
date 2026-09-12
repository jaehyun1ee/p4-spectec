//! Static assembly of the specification execution components
//!
//! `Runner<Interp, Iface, Exn>` owns the arena, specification, interpreter,
//! builtin interface, and extern implementation. The interpreter owns its
//! configuration and cache. Each evaluation borrows these components through
//! a context that also supports extern-to-interpreter reentry.

mod context;
mod externs;
mod interface;
mod interpreter;

use crate::lang::{
    data::value::{Value, ValueArena},
    il::ast::Typ,
};

pub use context::RunnerContext;
pub use externs::{Extern, ExternError, NullExtern};
pub use interface::{BuiltinInterface, Interface, InterfaceError, NullInterface};
pub use interpreter::Interpreter;

// == Runner assembly

/// An interpreter and its host components sharing one value arena
pub struct Runner<Interp, Iface, Exn>
where
    Interp: Interpreter<Iface, Exn>,
    Iface: Interface,
    Exn: Extern,
{
    arena: ValueArena,
    spec: Interp::Spec,
    interp: Interp,
    interface: Iface,
    external: Exn,
}

impl<Interp, Iface, Exn> Runner<Interp, Iface, Exn>
where
    Interp: Interpreter<Iface, Exn>,
    Iface: Interface,
    Exn: Extern,
{
    pub fn new(spec: Interp::Spec, interp: Interp, interface: Iface, external: Exn) -> Self {
        Self {
            arena: ValueArena::new(),
            spec,
            interp,
            interface,
            external,
        }
    }

    /// Borrows the assembled components for a stage-specific evaluation entry
    pub fn context(&mut self) -> RunnerContext<'_, Interp, Iface, Exn> {
        RunnerContext::new(
            &mut self.arena,
            &self.spec,
            &mut self.interp,
            &mut self.interface,
            &self.external,
        )
    }

    pub fn arena(&self) -> &ValueArena {
        &self.arena
    }

    pub fn arena_mut(&mut self) -> &mut ValueArena {
        &mut self.arena
    }

    // - Evaluation

    pub fn eval_program(
        &mut self,
        name: &str,
        program: Value,
    ) -> Result<Vec<Value>, Interp::Error> {
        let mut ctx = self.context();
        ctx.call_program(name, program)
    }

    pub fn eval_rel(&mut self, name: &str, values: &[Value]) -> Result<Vec<Value>, Interp::Error> {
        let mut ctx = self.context();
        ctx.call_rel(name, values)
    }

    pub fn eval_func(
        &mut self,
        name: &str,
        targs: &[Typ],
        values: &[Value],
    ) -> Result<Value, Interp::Error> {
        let mut ctx = self.context();
        ctx.call_func(name, targs, values)
    }

    // - Lifecycle

    /// Clears the interpreter cache and resets the builtin interface and externs
    pub fn clear(&mut self) {
        self.interp.clear();
        self.external.clear();
        self.interface.clear();
    }

    /// Starts an independent program while retaining definitions and configuration
    ///
    /// All previously returned arena handles become invalid. Call this before
    /// parsing the next program, after discarding the preceding program's values
    pub fn reset(&mut self) {
        self.interp.reset();
        self.external.clear();
        self.interface.clear();
        self.arena = ValueArena::new();
    }
}
