//! Errors produced while reading and rendering P4 programs.

use std::fmt;

use thiserror::Error;

use crate::lang::{common::source::Span, hints::alter::AlterationError};

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
    #[error(transparent)]
    Value(#[from] crate::lang::data::value::ValueError),
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

impl From<crate::lang::data::value::ValueError> for P4Error {
    fn from(error: crate::lang::data::value::ValueError) -> Self {
        Self::new(error, Span::default())
    }
}

impl fmt::Display for P4Error {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{} at {}", self.kind, self.span)
    }
}

impl std::error::Error for P4Error {}

#[derive(Clone, Debug, Error, PartialEq, Eq)]
pub enum P4UnparseError {
    #[error("cannot unparse runtime value kind {0}")]
    UnsupportedValue(&'static str),
    #[error(transparent)]
    Alteration(#[from] AlterationError),
}
