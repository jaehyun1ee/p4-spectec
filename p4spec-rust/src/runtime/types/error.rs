use thiserror::Error;

use crate::lang::common::source::Span;

/// A failure in a runtime type operation
#[derive(Clone, Debug, Error, PartialEq, Eq)]
#[error("{kind} at {span}")]
pub struct TypeError {
    pub kind: TypeErrorKind,
    pub span: Span,
}

impl TypeError {
    pub(crate) fn new(kind: TypeErrorKind, span: Span) -> Self {
        Self { kind, span }
    }
}

/// Category of a runtime type failure
#[derive(Clone, Debug, Error, PartialEq, Eq)]
pub enum TypeErrorKind {
    #[error("higher-order substitution is disallowed")]
    HigherOrderSubstitution,

    #[error("type argument count differs: expected {expected}, got {actual}")]
    TypeArgumentCount { expected: usize, actual: usize },

    #[error("type variable {0} is not defined")]
    UndefinedType(String),

    #[error("type parameter counts differ: {left} and {right}")]
    TypeParameterCount { left: usize, right: usize },

    #[error("parameter counts differ: {left} and {right}")]
    ParameterCount { left: usize, right: usize },
}
