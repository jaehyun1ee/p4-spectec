//! Typed failures produced while reading and parsing SpecTec source
//!
//! Lexical and syntax failures carry the span they were found at;
//! I/O and encoding failures carry the file they concern.

use std::{io, str::Utf8Error};

use thiserror::Error;

use crate::diagnostic::Report;
use crate::lang::common::source::Phrase;

/// A lexical failure category produced before parsing begins.
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

/// A lexical failure paired with the offending source span.
pub type LexError = Phrase<LexErrorKind>;

/// A syntax failure category independent of the parser implementation.
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
    #[error("expected notation type")]
    ExpectedNotationType,
    #[error("empty struct type")]
    EmptyStructType,
    #[error("empty variant type")]
    EmptyVariantType,
    #[error("empty type")]
    EmptyType,
    #[error("hints not allowed in plain type definition")]
    HintsInPlainTypeDefinition,
    #[error("empty syntax declaration")]
    EmptySyntaxDeclaration,
}

/// A syntax failure paired with the source span reported by the parser.
pub type SyntaxError = Phrase<SyntaxErrorKind>;

/// A UTF-8 decoding failure produced before parsing begins.
pub type InvalidUtf8Error = Utf8Error;

/// A failure from any stage of the SpecTec source frontend.
#[derive(Debug, Error)]
pub enum FrontendError {
    /// Carries migrated diagnostics until the frontend transition is complete.
    #[error(transparent)]
    Diagnostic(Box<Report>),
    /// The lexer rejected the source.
    #[error(transparent)]
    Lexical(#[from] LexError),
    /// The parser rejected the token stream.
    #[error(transparent)]
    Syntax(#[from] SyntaxError),
    /// A file could not be read.
    #[error("i/o error at {}: {}", .0.span, .0.node)]
    Io(#[source] Phrase<io::Error>),
    /// A file is not UTF-8; the span points at the first bad byte.
    #[error("source is not valid UTF-8 at {}: {}", .0.span, .0.node)]
    InvalidUtf8(#[source] Phrase<InvalidUtf8Error>),
}
