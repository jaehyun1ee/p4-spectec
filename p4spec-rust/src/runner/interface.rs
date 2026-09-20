//! Runner-facing interface for stateful specification builtins
//!
//! Calls are delegated to an interface implementation
//! and report whether they changed interface state.
//! For example, `$fresh_typeId` returns `true` with its fresh identifier,
//! while a pure builtin such as `$sum_nat` returns `false` with its value.
//! Failures remain independent of source locations;
//! the interpreter that evaluates a call owns that location.

use thiserror::Error;

use crate::{
    interface::builtin::{BuiltinError, call::Builtins},
    lang::data::value::{Value, ValueArena},
    lang::il::ast::{Id, Typ},
};

// == Interface errors

/// A failure inside the builtin interface.
#[derive(Clone, Debug, Error, Eq, PartialEq)]
pub enum InterfaceError {
    /// No interface is installed.
    #[error("interface is not configured")]
    NotConfigured,
    /// A builtin failed.
    #[error(transparent)]
    Builtin(#[from] Box<BuiltinError>),
}

// == Interface contract

/// Stateful specification builtins, called by name.
pub trait Interface {
    /// Calls a builtin; the flag reports an interface state change.
    fn call_builtin(
        &mut self,
        arena: &mut ValueArena,
        id: &Id,
        targs: &[Typ],
        values: &[Value],
    ) -> Result<(Value, bool), InterfaceError>;

    /// Resets builtin state between programs.
    fn clear(&mut self);
}

// == Standard implementations

/// The standard builtin table.
pub struct BuiltinInterface {
    builtins: Builtins,
}

impl BuiltinInterface {
    pub fn new(builtins: Builtins) -> Self {
        Self { builtins }
    }
}

impl Interface for BuiltinInterface {
    fn call_builtin(
        &mut self,
        arena: &mut ValueArena,
        id: &Id,
        targs: &[Typ],
        values: &[Value],
    ) -> Result<(Value, bool), InterfaceError> {
        self.builtins
            .invoke(arena, id, targs, values)
            .map_err(|error| InterfaceError::Builtin(Box::new(error)))
    }

    fn clear(&mut self) {
        self.builtins.init();
    }
}

/// An interface with no builtins; every call fails as not configured.
pub struct NullInterface;

impl Interface for NullInterface {
    fn call_builtin(
        &mut self,
        _arena: &mut ValueArena,
        _id: &Id,
        _targs: &[Typ],
        _values: &[Value],
    ) -> Result<(Value, bool), InterfaceError> {
        Err(InterfaceError::NotConfigured)
    }

    fn clear(&mut self) {}
}
