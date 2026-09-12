//! Typed failures produced during algorithm structuring

use thiserror::Error;

use crate::{
    lang::common::source::Span,
    runtime::ops::typ::{TypeError, TypeErrorKind},
};

/// Stable semantic category of a structuring failure
#[derive(Clone, Debug, Error, PartialEq, Eq)]
pub enum StructureErrorKind {
    #[error("cannot anti-unify expressions")]
    Antiunification,
    #[error("cannot populate anti-unified expressions")]
    TemplatePopulation,
    #[error("arity mismatch: expected {expected}, got {actual}")]
    ArityMismatch { expected: usize, actual: usize },
    #[error("incompatible anti-unification arguments")]
    IncompatibleArguments,
    #[error("identifier is unified in more than one input position")]
    ConflictingUnification,
    #[error("input hint operation failed: {0}")]
    Input(crate::lang::hints::input::InputError),
    #[error("type definition is undefined")]
    UndefinedType,
    #[error("meta-variable is undefined")]
    UndefinedMetavariable,
    #[error("type was already defined")]
    DuplicateType,
    #[error("meta-variable was already defined")]
    DuplicateMetavariable,
    #[error("type operation failed: {0}")]
    Type(TypeErrorKind),
}

/// A structuring failure paired with its source span
#[derive(Clone, Debug, Error, PartialEq, Eq)]
#[error("{kind} at {span}")]
pub struct StructureError {
    pub kind: StructureErrorKind,
    pub span: Span,
}

impl StructureError {
    pub(crate) fn new(kind: StructureErrorKind, span: Span) -> Self {
        Self { kind, span }
    }
}

impl From<TypeError> for StructureError {
    fn from(error: TypeError) -> Self {
        Self::new(StructureErrorKind::Type(error.kind), error.span)
    }
}
