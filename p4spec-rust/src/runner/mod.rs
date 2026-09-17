//! Static assembly of the specification execution components
//!
//! `Runner<Interp, Iface, Ext>` owns the arena, specification, interpreter,
//! builtin interface, and extern implementation. The interpreter owns its
//! configuration and cache. Each evaluation borrows these components through
//! a context that also supports extern-to-interpreter reentry.

mod context;
mod externs;
mod interface;
mod interpreter;

use crate::lang::data::value::{Value, ValueArena};

pub use context::RunnerContext;
pub use externs::{Extern, ExternError, NullExtern};
pub use interface::{BuiltinInterface, Interface, InterfaceError, NullInterface};
pub use interpreter::Interpreter;

// == Runner assembly

/// An interpreter and its host components sharing one value arena
pub struct Runner<Interp, Iface, Ext>
where
    Interp: Interpreter<Iface, Ext>,
    Iface: Interface,
    Ext: Extern,
{
    arena: ValueArena,
    spec: Interp::Spec,
    interp: Interp,
    interface: Iface,
    external: Ext,
}

impl<Interp, Iface, Ext> Runner<Interp, Iface, Ext>
where
    Interp: Interpreter<Iface, Ext>,
    Iface: Interface,
    Ext: Extern,
{
    pub fn new(spec: Interp::Spec, interp: Interp, interface: Iface, external: Ext) -> Self {
        Self { arena: ValueArena::new(), spec, interp, interface, external }
    }

    /// Borrows the assembled components for a stage-specific evaluation entry
    pub fn context(&mut self) -> RunnerContext<'_, Interp, Iface, Ext> {
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

    // - Lifecycle

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
