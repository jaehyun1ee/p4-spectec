//! Errors produced while evaluating specification builtins.

use thiserror::Error;

use crate::lang::data::value::ValueError;

#[derive(Clone, Debug, Error, Eq, PartialEq)]
pub enum BuiltinErrorKind {
    #[error("arity mismatch: expected {expected}, got {actual}")]
    ArityMismatch { expected: usize, actual: usize },

    #[error("implementation for builtin {0} is missing")]
    MissingImplementation(String),

    #[error("{0}")]
    InvalidArgument(String),

    #[error(transparent)]
    Value(#[from] ValueError),
}

#[derive(Clone, Debug, Error, Eq, PartialEq)]
#[error("{kind}")]
pub struct BuiltinError {
    pub kind: BuiltinErrorKind,
}

impl BuiltinError {
    pub fn new(message: impl Into<String>) -> Self {
        Self {
            kind: BuiltinErrorKind::InvalidArgument(message.into()),
        }
    }

    pub fn arity(expected: usize, actual: usize) -> Self {
        Self {
            kind: BuiltinErrorKind::ArityMismatch { expected, actual },
        }
    }
}
