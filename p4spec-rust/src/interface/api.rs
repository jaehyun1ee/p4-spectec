use thiserror::Error;

use crate::{
    domain::source::Region,
    lang::il::ast::{Id, Typ},
    runtime::value::ValueRef,
};

use super::builtin::{BuiltinError, call::Builtins};

#[derive(Clone, Debug, Error, Eq, PartialEq)]
#[error("{message} at {span}")]
pub struct InterfaceError {
    pub span: Region,
    pub message: String,
}

impl InterfaceError {
    pub fn new(span: Region, message: impl Into<String>) -> Self {
        Self {
            span,
            message: message.into(),
        }
    }
}

impl From<BuiltinError> for InterfaceError {
    fn from(error: BuiltinError) -> Self {
        Self::new(error.span, error.message)
    }
}

// Interface for the interaction between SpecTec and the defined language

pub trait Interface {
    // Builtins

    fn call_builtin(
        &mut self,
        add: &mut dyn FnMut(ValueRef),
        id: &Id,
        type_args: &[Typ],
        values: &[ValueRef],
    ) -> Result<ValueRef, InterfaceError>;

    // State management

    fn checkpoint(&self) -> u64;

    fn side_effected(&self, before: u64, after: u64) -> bool {
        before != after
    }

    // Clear the state

    fn clear(&mut self);
}

pub struct BuiltinInterface {
    builtins: Builtins,
}

impl BuiltinInterface {
    pub fn new() -> Self {
        Self {
            builtins: Builtins::new(),
        }
    }
}

impl Default for BuiltinInterface {
    fn default() -> Self {
        Self::new()
    }
}

impl Interface for BuiltinInterface {
    fn call_builtin(
        &mut self,
        add: &mut dyn FnMut(ValueRef),
        id: &Id,
        type_args: &[Typ],
        values: &[ValueRef],
    ) -> Result<ValueRef, InterfaceError> {
        self.builtins
            .invoke(add, id, type_args, values)
            .map_err(Into::into)
    }

    fn checkpoint(&self) -> u64 {
        self.builtins.checkpoint()
    }

    fn clear(&mut self) {
        self.builtins.init();
    }
}

pub struct NullInterface;

impl Interface for NullInterface {
    fn call_builtin(
        &mut self,
        _add: &mut dyn FnMut(ValueRef),
        id: &Id,
        _type_args: &[Typ],
        _values: &[ValueRef],
    ) -> Result<ValueRef, InterfaceError> {
        Err(InterfaceError::new(
            id.span.clone(),
            "interface is not configured",
        ))
    }

    fn checkpoint(&self) -> u64 {
        0
    }

    fn clear(&mut self) {}
}
