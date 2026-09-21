//! Errors produced while reading and rendering P4 programs
//!
//! Every failure of the P4 frontend becomes a `P4Error` with a span;
//! rendering failures are separate, since they arise inside a builtin.

use std::fmt;

use thiserror::Error;

use crate::lang::{common::source::Span, hints::alter::AlterationError};

/// A misuse of the parser's scope stack.
#[derive(Clone, Debug, Error, PartialEq, Eq)]
pub enum ContextError {
    /// No scope to declare into.
    #[error("P4 context has no scope")]
    MissingScope,
    /// The global scope was popped.
    #[error("cannot pop the root P4 scope")]
    RootScope,
}

/// A parse-tree value of an unexpected shape.
#[derive(Clone, Debug, Error, PartialEq, Eq)]
pub enum ExtractError {
    /// The named extractor met a value it has no case for.
    #[error("@{0}: unexpected value")]
    UnexpectedValue(&'static str),
}

/// A lexical failure in P4 source.
#[derive(Clone, Debug, Error, PartialEq, Eq)]
pub enum LexErrorKind {
    /// The source ended inside a string.
    #[error("unterminated string literal")]
    UnterminatedString,
    /// An escape other than `\"`, `\n`, or `\\`.
    #[error("unsupported escape sequence {0}")]
    UnsupportedEscape(String),
    /// The source ended inside `/* */`.
    #[error("unterminated block comment")]
    UnterminatedComment,
    /// Digits that do not parse in their radix.
    #[error("invalid integer literal {0}")]
    InvalidInteger(String),
    /// A `1s...` literal.
    #[error("signed integers must have width at least 2")]
    SignedWidth,
}

/// Why reading a P4 program failed.
#[derive(Clone, Debug, Error, PartialEq, Eq)]
pub enum P4ErrorKind {
    /// Building a parse-tree value failed.
    #[error(transparent)]
    Value(#[from] crate::lang::data::value::ValueError),
    /// `cc -E` failed or produced non-UTF-8 output.
    #[error("preprocessor failed with status {status:?}: {stderr}")]
    Preprocessor { status: Option<i32>, stderr: String },
    /// The scope stack was misused.
    #[error(transparent)]
    Context(#[from] ContextError),
    /// The lexer rejected the source.
    #[error(transparent)]
    Lex(#[from] LexErrorKind),
    /// The parser rejected the token stream.
    #[error("P4 syntax error")]
    Syntax,
}

/// A P4 frontend failure at a source span.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct P4Error {
    pub kind: P4ErrorKind,
    pub span: Span,
}

impl P4Error {
    /// An error of the given kind at `span`.
    pub fn new(kind: impl Into<P4ErrorKind>, span: Span) -> Self {
        Self { kind: kind.into(), span }
    }
}

impl fmt::Display for P4Error {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{} at {}", self.kind, self.span)
    }
}

impl std::error::Error for P4Error {}

/// Why rendering a value to P4 failed.
#[derive(Clone, Debug, Error, PartialEq, Eq)]
pub enum P4UnparseError {
    /// Structs, functions, and externs have no P4 spelling.
    #[error("cannot unparse runtime value kind {0}")]
    UnsupportedValue(&'static str),
    /// A print hint asked for an item that does not exist.
    #[error(transparent)]
    Alteration(#[from] AlterationError),
}

impl From<crate::lang::data::value::ValueError> for P4Error {
    fn from(error: crate::lang::data::value::ValueError) -> Self {
        Self::new(error, Span::default())
    }
}
