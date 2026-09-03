//! Runner-facing interface for stateful specification builtins.
//!
//! Calls are delegated to an interface implementation and report whether they
//! changed interface state. For example, `$fresh_typeId` returns `true` with
//! its fresh identifier, while a pure builtin such as `$sum_nat` returns
//! `false` with its value.

use std::rc::Rc;

use thiserror::Error;

use crate::{
    interface::builtin::{BuiltinError, call::Builtins},
    lang::{
        common::source::Span,
        il::ast::{Id, Typ},
    },
    runtime::value::Value,
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

// == Interface contract

pub trait Interface {
    fn call_builtin(
        &mut self,
        id: &Id,
        targs: &[Typ],
        values: &[Rc<Value>],
    ) -> Result<(Rc<Value>, bool), InterfaceError>;

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
        id: &Id,
        targs: &[Typ],
        values: &[Rc<Value>],
    ) -> Result<(Rc<Value>, bool), InterfaceError> {
        self.builtins
            .invoke(id, targs, values)
            .map_err(|error| InterfaceError {
                kind: InterfaceErrorKind::Builtin(Box::new(error)),
                span: id.span.clone(),
            })
    }

    fn clear(&mut self) {
        self.builtins.init();
    }
}

pub struct NullInterface;

impl Interface for NullInterface {
    fn call_builtin(
        &mut self,
        id: &Id,
        _targs: &[Typ],
        _values: &[Rc<Value>],
    ) -> Result<(Rc<Value>, bool), InterfaceError> {
        Err(InterfaceError {
            kind: InterfaceErrorKind::NotConfigured,
            span: id.span.clone(),
        })
    }

    fn clear(&mut self) {}
}
