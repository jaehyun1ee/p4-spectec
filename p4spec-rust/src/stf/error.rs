//! Typed STF frontend failures.

use std::{fmt, io};

use thiserror::Error;

use crate::lang::common::source::Span;

#[derive(Debug, Error)]
pub enum StfErrorKind {
    #[error("invalid character {0:?}")]
    InvalidCharacter(char),
    #[error("unterminated quoted identifier")]
    UnterminatedQuotedIdentifier,
    #[error("integer priority is out of range: {0}")]
    InvalidPriority(String),
    #[error("invalid numeric literal: {0}")]
    InvalidNumber(String),
    #[error("unexpected end of input")]
    UnexpectedEndOfInput,
    #[error("unexpected token")]
    UnexpectedToken,
    #[error("extra token")]
    ExtraToken,
    #[error("invalid token")]
    InvalidToken,
    #[error(transparent)]
    Io(#[from] io::Error),
}

#[derive(Debug)]
pub struct StfError {
    pub kind: StfErrorKind,
    pub span: Span,
}

impl StfError {
    pub fn new(kind: impl Into<StfErrorKind>, span: Span) -> Self {
        Self {
            kind: kind.into(),
            span,
        }
    }
}

impl fmt::Display for StfError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{} at {}", self.kind, self.span)
    }
}

impl std::error::Error for StfError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match &self.kind {
            StfErrorKind::Io(source) => Some(source),
            _ => None,
        }
    }
}
