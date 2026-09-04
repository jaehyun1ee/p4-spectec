//! Located errors produced by P4 preprocessing, lexing, and parsing.

use std::fmt;

use thiserror::Error;

use crate::lang::common::source::Span;

#[derive(Clone, Debug, Error, PartialEq, Eq)]
pub enum ContextError {
    #[error("P4 context has no scope")]
    MissingScope,
    #[error("cannot pop the root P4 scope")]
    RootScope,
}

#[derive(Clone, Debug, Error, PartialEq, Eq)]
pub enum ExtractError {
    #[error("@{0}: unexpected value")]
    UnexpectedValue(&'static str),
}

#[derive(Clone, Debug, Error, PartialEq, Eq)]
pub enum LexErrorKind {
    #[error("unterminated string literal")]
    UnterminatedString,
    #[error("unsupported escape sequence {0}")]
    UnsupportedEscape(String),
    #[error("unterminated block comment")]
    UnterminatedComment,
    #[error("invalid integer literal {0}")]
    InvalidInteger(String),
    #[error("signed integers must have width at least 2")]
    SignedWidth,
}

#[derive(Clone, Debug, Error, PartialEq, Eq)]
pub enum P4ErrorKind {
    #[error("preprocessor failed with status {status:?}: {stderr}")]
    Preprocessor { status: Option<i32>, stderr: String },
    #[error(transparent)]
    Context(#[from] ContextError),
    #[error(transparent)]
    Lex(#[from] LexErrorKind),
    #[error("P4 syntax error")]
    Syntax,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct P4Error {
    pub kind: P4ErrorKind,
    pub span: Span,
}

impl P4Error {
    pub fn new(kind: impl Into<P4ErrorKind>, span: Span) -> Self {
        Self {
            kind: kind.into(),
            span,
        }
    }
}

impl fmt::Display for P4Error {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{} at {}", self.kind, self.span)
    }
}

impl std::error::Error for P4Error {}
