//! Typed failures produced while reading and parsing SpecTec source

use std::{io, str::Utf8Error};

use thiserror::Error;

use crate::lang::common::source::Phrase;

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
pub type LexError = Phrase<LexErrorKind>;

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

/// A syntax failure paired with the source span reported by the parser
pub type SyntaxError = Phrase<SyntaxErrorKind>;

/// A UTF-8 decoding failure produced before parsing begins
pub type InvalidUtf8Error = Utf8Error;

/// A failure from any stage of the SpecTec source frontend
#[derive(Debug, Error)]
pub enum FrontendError {
    #[error(transparent)]
    Lexical(#[from] LexError),
    #[error(transparent)]
    Syntax(#[from] SyntaxError),
    #[error("i/o error at {}: {}", .0.span, .0.node)]
    Io(#[source] Phrase<io::Error>),
    #[error("source is not valid UTF-8 at {}: {}", .0.span, .0.node)]
    InvalidUtf8(#[source] Phrase<InvalidUtf8Error>),
}
