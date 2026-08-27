//! Typed failures produced while reading and parsing SpecTec source

use std::{io, str::Utf8Error};

use thiserror::Error;

use crate::lang::common::source::{Position, Span};

/// A lexical failure category produced before parsing begins
#[derive(Clone, Copy, Debug, Error, PartialEq, Eq)]
pub enum LexErrorKind {
    #[error("unclosed text literal")]
    UnclosedTextLiteral,
    #[error("illegal control character in text literal")]
    IllegalControlCharacter,
    #[error("illegal escape")]
    IllegalEscape,
    #[error("text literal is not valid UTF-8")]
    InvalidTextEncoding,
    #[error("unicode escape is outside the valid codepoint range")]
    InvalidUnicodeEscape,
    #[error("numbered hole is out of range")]
    HoleNumberOutOfRange,
    #[error("unclosed comment")]
    UnclosedComment,
    #[error("malformed token")]
    MalformedToken,
    #[error("misplaced control character")]
    MisplacedControlCharacter,
    #[error("misplaced unicode character")]
    MisplacedUnicodeCharacter,
}

/// A lexical failure paired with the offending source span
#[derive(Clone, Debug, Error, PartialEq, Eq)]
#[error("{kind} at {span}")]
pub struct LexError {
    pub kind: LexErrorKind,
    pub span: Span,
}

/// A syntax failure category independent of the parser implementation
#[derive(Clone, Copy, Debug, Error, PartialEq, Eq)]
pub enum SyntaxErrorKind {
    #[error("invalid token")]
    InvalidToken,
    #[error("unexpected end of input")]
    UnexpectedEndOfInput,
    #[error("unexpected token")]
    UnexpectedToken,
    #[error("extra token")]
    ExtraToken,
}

/// A syntax failure paired with the source span reported by the parser
#[derive(Clone, Debug, Error, PartialEq, Eq)]
#[error("{kind} at {span}")]
pub struct SyntaxError {
    pub kind: SyntaxErrorKind,
    pub span: Span,
}

impl SyntaxError {
    /// Constructs a syntax failure at an explicit source span
    pub fn new(kind: SyntaxErrorKind, span: Span) -> Self {
        Self { kind, span }
    }
}

/// A failure from any stage of the SpecTec source frontend
#[derive(Debug, Error)]
pub enum FrontendError {
    #[error(transparent)]
    Lexical(#[from] LexError),
    #[error(transparent)]
    Syntax(#[from] SyntaxError),
    #[error("i/o error at {span}: {source}")]
    Io {
        span: Span,
        #[source]
        source: io::Error,
    },
    #[error("source is not valid UTF-8 at {span}: {source}")]
    InvalidUtf8 {
        span: Span,
        #[source]
        source: Utf8Error,
    },
}

impl FrontendError {
    /// Constructs an I/O failure associated with a source file
    pub fn io(file: impl Into<String>, source: io::Error) -> Self {
        Self::Io {
            span: file_span(file),
            source,
        }
    }

    /// Constructs a UTF-8 decoding failure at a caller-computed source span
    pub fn invalid_utf8(span: Span, source: Utf8Error) -> Self {
        Self::InvalidUtf8 { span, source }
    }

    /// Returns the source span associated with this failure
    pub fn span(&self) -> &Span {
        match self {
            Self::Lexical(error) => &error.span,
            Self::Syntax(error) => &error.span,
            Self::Io { span, .. } | Self::InvalidUtf8 { span, .. } => span,
        }
    }
}

fn file_span(file: impl Into<String>) -> Span {
    let position = Position::new(file, 0, 0);
    Span::new(position.clone(), position)
}
