//! Errors produced while evaluating specification builtins
//!
//! A builtin failure is recoverable to the interpreter,
//! which tries the next candidate;
//! value errors inside a builtin are wrapped the same way.

use thiserror::Error;

use crate::{interface::p4::error::P4UnparseError, lang::data::value::ValueError};

/// Why a builtin call failed.
#[derive(Clone, Debug, Error, Eq, PartialEq)]
pub enum BuiltinErrorKind {
    /// Wrong number of type arguments or values.
    #[error("arity mismatch: expected {expected}, got {actual}")]
    ArityMismatch { expected: usize, actual: usize },

    /// The specification declares a builtin this interface lacks.
    #[error("implementation for builtin {0} is missing")]
    MissingImplementation(String),

    /// An argument had the right kind but an unusable value.
    #[error("{0}")]
    InvalidArgument(String),

    /// A value projection or construction failed.
    #[error(transparent)]
    Value(#[from] ValueError),

    /// `print_` could not render the value.
    #[error(transparent)]
    P4Unparse(#[from] P4UnparseError),
}

/// A builtin failure.
#[derive(Clone, Debug, Error, Eq, PartialEq)]
#[error("{kind}")]
pub struct BuiltinError {
    pub kind: BuiltinErrorKind,
}

impl BuiltinError {
    /// An invalid-argument failure with a message.
    pub fn new(message: impl Into<String>) -> Self {
        Self { kind: BuiltinErrorKind::InvalidArgument(message.into()) }
    }

    /// An arity failure.
    pub fn arity(expected: usize, actual: usize) -> Self {
        Self { kind: BuiltinErrorKind::ArityMismatch { expected, actual } }
    }
}

impl From<ValueError> for BuiltinError {
    fn from(error: ValueError) -> Self {
        Self { kind: BuiltinErrorKind::Value(error) }
    }
}
