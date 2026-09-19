//! Typed failures produced during prose conversion

use thiserror::Error;

use crate::lang::{
    common::source::Span,
    hints::{alter::AlterationError, fields::FieldError, input::InputError},
};

/// Stable semantic category of a prose-conversion failure
#[derive(Clone, Debug, Error, PartialEq, Eq)]
pub enum ProseErrorKind {
    #[error("invalid expression for prose hint `{0}`")]
    InvalidHintExpression(String),
    #[error("alteration hint is invalid: {0}")]
    Alteration(AlterationError),
    #[error("field hint is invalid: {0}")]
    Field(FieldError),
    #[error("input hint operation failed: {0}")]
    Input(InputError),
    #[error("a result, return, or rule application cannot appear at dispatch level")]
    InvalidDispatchTier,
    #[error("a rule group cannot appear in a group body")]
    InvalidGroupTier,
}

/// A prose-conversion failure paired with its source span
#[derive(Clone, Debug, Error, PartialEq, Eq)]
#[error("{kind} at {span}")]
pub struct ProseError {
    pub kind: ProseErrorKind,
    pub span: Span,
}

impl ProseError {
    pub(crate) fn new(kind: ProseErrorKind, span: Span) -> Self {
        Self { kind, span }
    }
}
