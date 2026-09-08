//! Runner-facing interface for stateful specification builtins.
//!
//! Calls are delegated to an interface implementation and report whether they
//! changed interface state. For example, `$fresh_typeId` returns `true` with
//! its fresh identifier, while a pure builtin such as `$sum_nat` returns
//! `false` with its value. Failures remain independent of source locations;
//! the interpreter that evaluates a call owns that location.

use thiserror::Error;

use crate::{
    interface::{
        builtin::{BuiltinError, BuiltinErrorKind, call::Builtins, extract},
        p4::unparse::P4Unparser,
    },
    lang::common::source::Span,
    lang::data::value::{self, Value, ValueArena},
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
        arena: &mut ValueArena,
        id: &Id,
        targs: &[Typ],
        values: &[Value],
    ) -> Result<(Value, bool), InterfaceError>;

    fn clear(&mut self);
}

// == Standard implementations

pub struct BuiltinInterface {
    builtins: Builtins,
    unparser: P4Unparser,
}

impl BuiltinInterface {
    pub fn new(unparser: P4Unparser) -> Self {
        Self {
            builtins: Builtins::new(),
            unparser,
        }
    }

    fn print(
        &self,
        arena: &mut ValueArena,
        targs: &[Typ],
        values: &[Value],
    ) -> Result<Value, BuiltinError> {
        let _typ = extract::one(targs)?;
        let value = extract::one(values)?;
        let text = self
            .unparser
            .render(arena, value)
            .map_err(|error| BuiltinError {
                kind: BuiltinErrorKind::P4Unparse(error),
            })?;
        Ok(value::make::text(arena, text, Span::default())?)
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
        let result = match id.node.as_str() {
            "print_" => self.print(arena, targs, values).map(|value| (value, false)),
            _ => self.builtins.invoke(arena, id, targs, values),
        };
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
        _arena: &mut ValueArena,
        _id: &Id,
        _targs: &[Typ],
        _values: &[Value],
    ) -> Result<(Value, bool), InterfaceError> {
        Err(InterfaceError::NotConfigured)
    }

    fn clear(&mut self) {}
}
