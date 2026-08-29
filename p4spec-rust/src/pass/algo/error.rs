//! Typed failures produced during algorithmic conversion

use thiserror::Error;

use crate::lang::{common::source::Span, hints::input::InputError};
use crate::runtime::types::{TypeError, TypeErrorKind};

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
    #[error("type argument arity mismatch: expected {expected}, got {actual}")]
    TypeArgumentArityMismatch { expected: usize, actual: usize },
    #[error("type operation failed: {0}")]
    Type(TypeErrorKind),
    #[error("cannot anti-unify expressions")]
    AntiUnification,
    #[error("expression arity mismatch: expected {expected}, got {actual}")]
    ExpressionArityMismatch { expected: usize, actual: usize },
    #[error("invalid relation input hint: {0}")]
    InputHint(InputError),
    #[error("expression contains unbound identifiers")]
    FreeBindings,
    #[error("bindings are not shallow")]
    BindingsNotShallow,
    #[error("shallow bindings generated repeated-binding side conditions")]
    ShallowSideConditions,
    #[error("cannot bind on both sides of an equality")]
    BindingOnBothEqualitySides,
    #[error("let premise is invalid before binding analysis")]
    UnexpectedLetPremise,
    #[error("iterated premise has binding variables before binding analysis")]
    UnexpectedIterationBindings,
    #[error("otherwise branch contains an impure premise")]
    ImpureElsePremises,
    #[error("table pattern type is not a defined variant")]
    NonVariantPatternType,
    #[error("table row contains an unsupported pattern expression")]
    InvalidTablePattern,
    #[error("table declaration contains a non-expression parameter")]
    InvalidTableParameter,
    #[error("table rows have overlapping patterns")]
    OverlappingTablePatterns,
    #[error("table rows are missing patterns")]
    MissingTablePatterns,
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

impl From<TypeError> for AlgoError {
    fn from(error: TypeError) -> Self {
        Self::new(AlgoErrorKind::Type(error.kind), error.span)
    }
}
