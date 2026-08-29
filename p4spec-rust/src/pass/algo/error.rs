//! Typed failures produced during algorithmic conversion

use thiserror::Error;

use crate::lang::common::source::Span;

/// Stable semantic category of an algorithmic-conversion failure
#[derive(Clone, Debug, Error, PartialEq, Eq)]
pub enum AlgoErrorKind {
    #[error("algorithmic conversion is not implemented")]
    Unsupported,
    #[error("type definition is undefined")]
    UndefinedType,
    #[error("inconsistent dimensions for multiple bindings")]
    InconsistentDimensions,
    #[error("invalid binding position in non-invertible {0}")]
    NonInvertibleBinding(&'static str),
    #[error("empty iteration")]
    EmptyIteration,
    #[error("cannot determine the dimension of binding-only identifiers")]
    UndeterminedBindingDimension,
    #[error("pattern arity mismatch: expected {expected}, got {actual}")]
    PatternArityMismatch { expected: usize, actual: usize },
}

/// An algorithmic-conversion failure paired with its source span
#[derive(Clone, Debug, Error, PartialEq, Eq)]
#[error("{kind} at {span}")]
pub struct AlgoError {
    pub kind: AlgoErrorKind,
    pub span: Span,
}

impl AlgoError {
    pub(crate) fn new(kind: AlgoErrorKind, span: Span) -> Self {
        Self { kind, span }
    }
}
