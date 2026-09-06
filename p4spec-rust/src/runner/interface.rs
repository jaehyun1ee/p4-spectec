//! Runner-facing interface for stateful specification builtins.
//!
//! Calls are delegated to an interface implementation and report whether they
//! changed interface state. For example, `$fresh_typeId` returns `true` with
//! its fresh identifier, while a pure builtin such as `$sum_nat` returns
//! `false` with its value. Failures remain independent of source locations;
//! the interpreter that evaluates a call owns that location.

use std::rc::Rc;

use thiserror::Error;

use crate::{
    interface::builtin::{BuiltinError, call::Builtins},
    lang::data::value::Value,
    lang::il::ast::{Id, Typ},
};

// == Interface errors

#[derive(Clone, Debug, Error, Eq, PartialEq)]
pub enum InterfaceError {
    #[error("interface is not configured")]
    NotConfigured,
    #[error(transparent)]
    Builtin(#[from] Box<BuiltinError>),
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
        let result = self.builtins.invoke(id, targs, values);
        result.map_err(|error| InterfaceError::Builtin(Box::new(error)))
    }

    fn clear(&mut self) {
        self.builtins.init();
    }
}

pub struct NullInterface;

impl Interface for NullInterface {
    fn call_builtin(
        &mut self,
        _id: &Id,
        _targs: &[Typ],
        _values: &[Rc<Value>],
    ) -> Result<(Rc<Value>, bool), InterfaceError> {
        Err(InterfaceError::NotConfigured)
    }

    fn clear(&mut self) {}
}
