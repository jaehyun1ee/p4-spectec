//! Runtime implementations of the standard SpecTec builtins.
//!
//! The dispatcher validates arity, then each family decodes its arguments and
//! computes one result. For example, `$sum_int([2, 5])` returns the value `7`.

use std::rc::Rc;

use thiserror::Error;

use crate::{
    lang::common::source::Span,
    runtime::value::{Value, ValueError},
};

pub mod call;
pub mod extract;
pub mod fresh;
pub mod ints;
pub mod lists;
pub mod maps;
pub mod nats;
pub mod numerics;
pub mod sets;
pub mod texts;

// == Errors

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
#[error("{kind} at {span}")]
pub struct BuiltinError {
    pub kind: BuiltinErrorKind,
    pub span: Span,
}

impl BuiltinError {
    pub fn new(span: Span, message: impl Into<String>) -> Self {
        Self {
            kind: BuiltinErrorKind::InvalidArgument(message.into()),
            span,
        }
    }

    pub fn arity(span: Span, expected: usize, actual: usize) -> Self {
        Self {
            kind: BuiltinErrorKind::ArityMismatch { expected, actual },
            span,
        }
    }
}

pub type BuiltinResult = Result<Rc<Value>, BuiltinError>;
