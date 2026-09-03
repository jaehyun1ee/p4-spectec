//! Runner-facing interface for stateful specification builtins.
//!
//! Calls are delegated to an interface implementation, while checkpoints let
//! the runner detect effects around speculative evaluation. For example, the
//! builtin interface changes its checkpoint after `$fresh_typeId`.

use thiserror::Error;

use crate::{
    interface::builtin::{BuiltinError, call::Builtins},
    lang::{
        common::source::Span,
        il::ast::{Id, Typ},
    },
    runtime::value::ValueRef,
};

// == Interface errors

#[derive(Clone, Debug, Error, Eq, PartialEq)]
pub enum InterfaceErrorKind {
    #[error("interface is not configured")]
    NotConfigured,
    #[error(transparent)]
    Builtin(#[from] Box<BuiltinError>),
}

#[derive(Clone, Debug, Error, Eq, PartialEq)]
#[error("{kind} at {span}")]
pub struct InterfaceError {
    pub kind: InterfaceErrorKind,
    pub span: Span,
}

impl From<BuiltinError> for InterfaceError {
    fn from(error: BuiltinError) -> Self {
        Self {
            span: error.span.clone(),
            kind: InterfaceErrorKind::Builtin(Box::new(error)),
        }
    }
}

// == Interface contract

pub trait Interface {
    fn call_builtin(
        &mut self,
        add: &mut dyn FnMut(ValueRef),
        id: &Id,
        targs: &[Typ],
        values: &[ValueRef],
    ) -> Result<ValueRef, InterfaceError>;

    fn checkpoint(&self) -> u64;

    fn side_effected(&self, before: u64, after: u64) -> bool {
        before != after
    }

    fn clear(&mut self);
}

// == Standard implementations

#[derive(Default)]
pub struct BuiltinInterface {
    builtins: Builtins,
}

impl BuiltinInterface {
    pub fn new() -> Self {
        Self::default()
    }
}

impl Interface for BuiltinInterface {
    fn call_builtin(
        &mut self,
        add: &mut dyn FnMut(ValueRef),
        id: &Id,
        targs: &[Typ],
        values: &[ValueRef],
    ) -> Result<ValueRef, InterfaceError> {
        self.builtins
            .invoke(add, id, targs, values)
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
        _targs: &[Typ],
        _values: &[ValueRef],
    ) -> Result<ValueRef, InterfaceError> {
        Err(InterfaceError {
            kind: InterfaceErrorKind::NotConfigured,
            span: id.span.clone(),
        })
    }

    fn checkpoint(&self) -> u64 {
        0
    }

    fn clear(&mut self) {}
}
