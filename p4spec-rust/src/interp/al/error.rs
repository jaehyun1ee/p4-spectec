//! Located failures in AL definition lookup and iteration

use std::fmt;

use crate::lang::{common::source::Span, data::value::ValueError};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EntityKind {
    Value,
    Type,
    DefinedType,
    Relation,
    Function,
}

impl fmt::Display for EntityKind {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::Value => "value",
            Self::Type => "type",
            Self::DefinedType => "defined type",
            Self::Relation => "relation",
            Self::Function => "function",
        })
    }
}

#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
pub enum ErrorKind {
    #[error("{kind} `{name}` is undefined")]
    Undefined { kind: EntityKind, name: String },
    #[error("{kind} `{name}` was already defined")]
    Duplicate { kind: EntityKind, name: String },
    #[error("mismatch in optionality of iterated variables")]
    OptionalityMismatch,
    #[error("cannot transpose a matrix of value batches")]
    IterationLengthMismatch { expected: usize, actual: usize },
    #[error(transparent)]
    Value(#[from] ValueError),
}

#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
#[error("{kind} at {span}")]
pub struct Error {
    pub kind: ErrorKind,
    pub span: Span,
}

impl Error {
    pub fn new(kind: ErrorKind, span: Span) -> Self {
        Self { kind, span }
    }

    pub(super) fn undefined(kind: EntityKind, name: String, span: Span) -> Self {
        Self::new(ErrorKind::Undefined { kind, name }, span)
    }

    pub(super) fn duplicate(kind: EntityKind, name: String, span: Span) -> Self {
        Self::new(ErrorKind::Duplicate { kind, name }, span)
    }
}
