//! Runtime implementations of the standard SpecTec builtins.
//!
//! The dispatcher validates arity, each family decodes and computes its
//! result, and `return_value` registers the final value with the runner. Thus a
//! call such as `$sum_int([2, 5])` both returns and records the same value `7`.

use std::rc::Rc;

use thiserror::Error;

use crate::{
    lang::common::source::Span,
    runtime::value::{ValueError, ValueRef},
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

pub type BuiltinResult = Result<ValueRef, BuiltinError>;

// == Result registration

pub(crate) fn return_value(add: &mut dyn FnMut(ValueRef), value: ValueRef) -> BuiltinResult {
    add(Rc::clone(&value));
    Ok(value)
}
