//! Lazy tokenization of SpecTec source text
//!
//! Source and decoded text literals are UTF-8 strings. Hex byte escapes may
//! combine into a valid UTF-8 sequence; byte-only results are rejected with
//! [`LexErrorKind::InvalidTextEncoding`] so tokens fit the language model's
//! `String` text representation.

use num_bigint::BigInt;
use thiserror::Error;

use crate::lang::{
    common::source::{Position, Span, Spanned},
    xl::{num::Natural, utf8},
};

/// A token consumed by the SpecTec grammar
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Token {
    TagUpperId(String),
    Operator(String),
    TickLeftParen,
    TickRightParen,
    TickLeftBracket,
    TickRightBracket,
    TickLeftBrace,
    TickRightBrace,
    TickLeftAngle,
    TickRightAngle,
    NewlineBar,
    Newline2,
    Newline3,
    Subtype,
    Turnstile,
    Tilesturn,
    Arrow,
    ArrowSub,
    DoubleArrow,
    DoubleArrowSub,
    DoubleArrowBoth,
    DoubleArrowLong,
    SquigglyArrow,
    SquigglyArrowStar,
    And,
    Or,
    Dot,
    DoubleDot,
    TripleDot,
    Comma,
    CommaNewline,
    Semicolon,
    Colon,
    DoubleColon,
    ColonSlash,
    ColonEquals,
    Hash,
    DoubleHash,
    Dollar,
    Question,
    Tilde,
    DoubleTilde,
    LeftAngle,
    LeftAngleDash,
    LeftAngleEquals,
    RightAngle,
    RightAngleEquals,
    RightAngleLeftParen,
    LeftParen,
    RightParen,
    LeftBracket,
    RightBracket,
    LeftBrace,
    RightBrace,
    Plus,
    DoublePlus,
    Minus,
    Dash,
    Star,
    Slash,
    Backslash,
    Hole,
    NumberedHole(i64),
    MultipleHole,
    EmptyHole,
    Equals,
    NotEquals,
    Up,
    Bar,
    Latex,
    Bool,
    Nat,
    Int,
    Text,
    Syntax,
    Extern,
    Table,
    Relation,
    RuleGroup,
    Rule,
    Var,
    Builtin,
    Dec,
    Def,
    If,
    Otherwise,
    Debug,
    HintLeftParen,
    Epsilon,
    BoolLiteral(bool),
    NaturalLiteral(Natural),
    HexLiteral(Natural),
    TextLiteral(String),
    UpperId(String),
    LowerId(String),
    DotId(String),
    UpperIdLeftParen(String),
    LowerIdLeftParen(String),
    UpperIdLeftAngle(String),
    LowerIdLeftAngle(String),
    Eof,
}

/// The lexical failure categories produced before parsing begins
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

/// A typed lexical failure paired with the offending source span
#[derive(Clone, Debug, Error, PartialEq, Eq)]
#[error("{kind} at {span}")]
pub struct LexError {
    pub kind: LexErrorKind,
    pub span: Span,
}

#[derive(Clone, Copy)]
struct Cursor {
    offset: usize,
    line: i64,
    line_start: usize,
}

fn never_uppercase_variable(_identifier: &str) -> bool {
    false
}

/// A lazy SpecTec token stream
///
/// The classifier relabels uppercase identifiers that are variables in the
/// parser's current scope. The default constructor classifies all uppercase
/// identifiers as atoms. Input is valid UTF-8 by construction; file entry
/// points must report decoding failures before constructing a lexer.
pub struct Lexer<'input, Classify = fn(&str) -> bool> {
    file: String,
    source: &'input str,
    cursor: Cursor,
    finished: bool,
    classify_uppercase: Classify,
}

impl<'input> Lexer<'input> {
    /// Tokenizes `source` with no parser-owned variable scope
    pub fn new(file: impl Into<String>, source: &'input str) -> Self {
        Self::with_uppercase_classifier(file, source, never_uppercase_variable)
    }

    /// Tokenizes `source` using the parser's uppercase-variable classifier
    pub fn with_uppercase_classifier<Classify>(
        file: impl Into<String>,
        source: &'input str,
        classify_uppercase: Classify,
    ) -> Lexer<'input, Classify>
    where
        Classify: FnMut(&str) -> bool,
    {
        Lexer {
            file: file.into(),
            source,
            cursor: Cursor {
                offset: 0,
                line: 1,
                line_start: 0,
            },
            finished: false,
            classify_uppercase,
        }
    }
}

impl<Classify> Lexer<'_, Classify>
where
    Classify: FnMut(&str) -> bool,
{
    fn next_lexeme(&mut self) -> Result<Spanned<Token>, LexError> {
        loop {
            let start = self.cursor;
            if self.is_eof() {
                return Ok(self.lexeme(Token::Eof, start));
            }

            if self.starts_with("(;") {
                self.advance_ascii(2);
                self.skip_block_comment(start)?;
                continue;
            }

            if self.starts_with(";;") {
                if let Some(lexeme) = self.consume_line_comment(start)? {
                    return Ok(lexeme);
                }
                continue;
            }

            if self.starts_with("\\\n") {
                self.advance_ascii(1);
                self.advance_newline();
                continue;
            }

            match self.current_byte() {
                Some(b'\n') => {
                    self.advance_newline();
                    if let Some(lexeme) = self.after_newline()? {
                        return Ok(lexeme);
                    }
                    continue;
                }
                Some(b' ' | b'\t' | b'\r') => {
                    self.advance_ascii(1);
                    continue;
                }
                Some(b'"') => return self.scan_text(start),
                Some(b'\'') => return self.scan_operator(start),
                _ => {}
            }

            if let Some(lexeme) = self.scan_comma_newline(start) {
                return Ok(lexeme);
            }
            if let Some(lexeme) = self.scan_tag(start) {
                return Ok(lexeme);
            }
            if let Some(lexeme) = self.scan_dot_identifier(start) {
                return Ok(lexeme);
            }
            if let Some(lexeme) = self.scan_numbered_hole(start)? {
                return Ok(lexeme);
            }
            if let Some(lexeme) = self.scan_number(start) {
                return Ok(lexeme);
            }
            if let Some(lexeme) = self.scan_identifier(start) {
                return Ok(lexeme);
            }
            if let Some(lexeme) = self.scan_fixed(start) {
                return Ok(lexeme);
            }

            return Err(self.unrecognized_character(start));
        }
    }

    fn consume_line_comment(&mut self, start: Cursor) -> Result<Option<Spanned<Token>>, LexError> {
        self.advance_to_newline_or_eof();
        if self.is_eof() {
            return Ok(Some(self.lexeme(Token::Eof, start)));
        }

        self.advance_newline();
        self.after_newline()
    }

    fn after_newline(&mut self) -> Result<Option<Spanned<Token>>, LexError> {
        if let Some(lexeme) = self.scan_newline_bar() {
            return Ok(Some(lexeme));
        }

        let end = self.indentation_end();
        if self.byte_at(end) == Some(b'\n') {
            self.advance_to(end);
            self.advance_newline();
            return self.after_two_newlines().map(Some);
        }

        Ok(None)
    }

    fn after_two_newlines(&mut self) -> Result<Spanned<Token>, LexError> {
        loop {
            if let Some(lexeme) = self.scan_newline_bar() {
                return Ok(lexeme);
            }

            let start = self.cursor;
            let indent_end = self.indentation_end();
            if self.byte_at(indent_end) == Some(b'\n') {
                self.advance_to(indent_end);
                self.advance_newline();
                return Ok(self.lexeme(Token::Newline3, start));
            }

            if self.source[indent_end..].starts_with(";;") {
                let line_end = self.newline_or_eof_from(indent_end);
                if self.byte_at(line_end) == Some(b'\n') {
                    self.advance_to(line_end);
                    self.advance_newline();
                    continue;
                }
                self.advance_to(line_end);
                return Ok(self.lexeme(Token::Eof, start));
            }

            if indent_end == self.source.len() {
                self.advance_to(indent_end);
                return Ok(self.lexeme(Token::Eof, start));
            }

            return Ok(self.lexeme(Token::Newline2, self.cursor));
        }
    }

    fn scan_newline_bar(&mut self) -> Option<Spanned<Token>> {
        let start = self.cursor;
        let indent_end = self.indentation_end();
        if self.byte_at(indent_end) != Some(b'|')
            || !matches!(self.byte_at(indent_end + 1), Some(b' ' | b'\t'))
        {
            return None;
        }

        self.advance_to(indent_end + 2);
        Some(self.lexeme(Token::NewlineBar, start))
    }

    fn scan_comma_newline(&mut self, start: Cursor) -> Option<Spanned<Token>> {
        if self.current_byte() != Some(b',') {
            return None;
        }

        let mut end = self.cursor.offset + 1;
        while matches!(self.byte_at(end), Some(b' ' | b'\t')) {
            end += 1;
        }
        if self.source[end..].starts_with(";;") {
            end = self.newline_or_eof_from(end);
        }
        if self.byte_at(end) != Some(b'\n') {
            return None;
        }

        self.advance_to(end);
        self.advance_newline();
        Some(self.lexeme(Token::CommaNewline, start))
    }

    fn scan_tag(&mut self, start: Cursor) -> Option<Spanned<Token>> {
        if self.current_byte() != Some(b'_') || !is_upper(self.byte_at(self.cursor.offset + 1)?) {
            return None;
        }

        let id_start = self.cursor.offset + 1;
        let end = self.identifier_end(id_start);
        if matches!(self.byte_at(end), Some(b'(' | b'<')) {
            return None;
        }
        let identifier = self.source[id_start..end].to_owned();
        self.advance_to(end);
        Some(self.lexeme(Token::TagUpperId(identifier), start))
    }

    fn scan_dot_identifier(&mut self, start: Cursor) -> Option<Spanned<Token>> {
        if self.current_byte() != Some(b'.')
            || !is_identifier_start(self.byte_at(self.cursor.offset + 1)?)
        {
            return None;
        }

        let id_start = self.cursor.offset + 1;
        let end = self.identifier_end(id_start);
        if end - start.offset <= 3 && self.source[start.offset..].starts_with("...") {
            return None;
        }

        let identifier = self.source[id_start..end].to_owned();
        self.advance_to(end);
        Some(self.lexeme(Token::DotId(identifier), start))
    }

    fn scan_numbered_hole(&mut self, start: Cursor) -> Result<Option<Spanned<Token>>, LexError> {
        if self.current_byte() != Some(b'%')
            || !is_digit(self.byte_at(self.cursor.offset + 1).unwrap_or_default())
        {
            return Ok(None);
        }

        let end = self.separated_digits_end(self.cursor.offset + 1, is_digit);
        let digits = without_underscores(&self.source[self.cursor.offset + 1..end]);
        self.advance_to(end);
        let number = digits
            .parse::<i64>()
            .ok()
            .filter(|number| *number <= OCAML_INT_MAX)
            .ok_or_else(|| self.error(LexErrorKind::HoleNumberOutOfRange, start))?;
        Ok(Some(self.lexeme(Token::NumberedHole(number), start)))
    }

    fn scan_number(&mut self, start: Cursor) -> Option<Spanned<Token>> {
        if !is_digit(self.current_byte()?) {
            return None;
        }

        if self.starts_with("0x")
            && self
                .byte_at(self.cursor.offset + 2)
                .is_some_and(is_hex_digit)
        {
            let end = self.separated_digits_end(self.cursor.offset + 2, is_hex_digit);
            let digits = without_underscores(&self.source[self.cursor.offset + 2..end]);
            let natural = parse_natural(&digits, 16);
            self.advance_to(end);
            return Some(self.lexeme(Token::HexLiteral(natural), start));
        }

        let end = self.separated_digits_end(self.cursor.offset, is_digit);
        let digits = without_underscores(&self.source[self.cursor.offset..end]);
        let natural = parse_natural(&digits, 10);
        self.advance_to(end);
        Some(self.lexeme(Token::NaturalLiteral(natural), start))
    }

    fn scan_identifier(&mut self, start: Cursor) -> Option<Spanned<Token>> {
        let first = self.current_byte()?;
        if !is_identifier_start(first) {
            return None;
        }

        let is_uppercase = is_upper(first);
        let end = self.identifier_end(self.cursor.offset);
        let identifier = self.source[self.cursor.offset..end].to_owned();
        let suffix = self.byte_at(end);
        if identifier == "hint" && suffix == Some(b'(') {
            self.advance_to(end + 1);
            return Some(self.lexeme(Token::HintLeftParen, start));
        }

        let uppercase_variable = is_uppercase && (self.classify_uppercase)(&identifier);
        let token = match suffix {
            Some(b'(') => {
                self.advance_to(end + 1);
                if is_uppercase && !uppercase_variable {
                    Token::UpperIdLeftParen(identifier)
                } else {
                    Token::LowerIdLeftParen(identifier)
                }
            }
            Some(b'<') => {
                self.advance_to(end + 1);
                if is_uppercase && !uppercase_variable {
                    Token::UpperIdLeftAngle(identifier)
                } else {
                    Token::LowerIdLeftAngle(identifier)
                }
            }
            _ => {
                self.advance_to(end);
                if let Some(keyword) = keyword(&identifier) {
                    keyword
                } else if is_uppercase && !uppercase_variable {
                    Token::UpperId(identifier)
                } else {
                    Token::LowerId(identifier)
                }
            }
        };

        Some(self.lexeme(token, start))
    }

    fn scan_fixed(&mut self, start: Cursor) -> Option<Spanned<Token>> {
        let (length, token) = if self.starts_with("->_") {
            (3, Token::ArrowSub)
        } else if self.starts_with("=>_") {
            (3, Token::DoubleArrowSub)
        } else if self.starts_with("<=>") {
            (3, Token::DoubleArrowBoth)
        } else if self.starts_with("==>") {
            (3, Token::DoubleArrowLong)
        } else if self.starts_with("~>*") {
            (3, Token::SquigglyArrowStar)
        } else if self.starts_with("=/=") {
            (3, Token::NotEquals)
        } else if self.starts_with("%latex") {
            (6, Token::Latex)
        } else if self.starts_with("`(") {
            (2, Token::TickLeftParen)
        } else if self.starts_with("`)") {
            (2, Token::TickRightParen)
        } else if self.starts_with("`[") {
            (2, Token::TickLeftBracket)
        } else if self.starts_with("`]") {
            (2, Token::TickRightBracket)
        } else if self.starts_with("`{") {
            (2, Token::TickLeftBrace)
        } else if self.starts_with("`}") {
            (2, Token::TickRightBrace)
        } else if self.starts_with("`<") {
            (2, Token::TickLeftAngle)
        } else if self.starts_with("`>") {
            (2, Token::TickRightAngle)
        } else if self.starts_with("|-") {
            (2, Token::Turnstile)
        } else if self.starts_with("-|") {
            (2, Token::Tilesturn)
        } else if self.starts_with("->") {
            (2, Token::Arrow)
        } else if self.starts_with("=>") {
            (2, Token::DoubleArrow)
        } else if self.starts_with("~>") {
            (2, Token::SquigglyArrow)
        } else if self.starts_with("/\\") {
            (2, Token::And)
        } else if self.starts_with("\\/") {
            (2, Token::Or)
        } else if self.starts_with("...") {
            (3, Token::TripleDot)
        } else if self.starts_with("..") {
            (2, Token::DoubleDot)
        } else if self.starts_with("::") {
            (2, Token::DoubleColon)
        } else if self.starts_with(":/") {
            (2, Token::ColonSlash)
        } else if self.starts_with(":=") {
            (2, Token::ColonEquals)
        } else if self.starts_with("##") {
            (2, Token::DoubleHash)
        } else if self.starts_with("<:") {
            (2, Token::Subtype)
        } else if self.starts_with("~~") {
            (2, Token::DoubleTilde)
        } else if self.starts_with("<-") {
            (2, Token::LeftAngleDash)
        } else if self.starts_with("<=") {
            (2, Token::LeftAngleEquals)
        } else if self.starts_with(">=") {
            (2, Token::RightAngleEquals)
        } else if self.starts_with(">(") {
            (2, Token::RightAngleLeftParen)
        } else if self.starts_with("++") {
            (2, Token::DoublePlus)
        } else if self.starts_with("--") {
            (2, Token::Dash)
        } else if self.starts_with("%%") {
            (2, Token::MultipleHole)
        } else if self.starts_with("!%") {
            (2, Token::EmptyHole)
        } else {
            let token = match self.current_byte()? {
                b'.' => Token::Dot,
                b',' => Token::Comma,
                b';' => Token::Semicolon,
                b':' => Token::Colon,
                b'#' => Token::Hash,
                b'$' => Token::Dollar,
                b'?' => Token::Question,
                b'~' => Token::Tilde,
                b'<' => Token::LeftAngle,
                b'>' => Token::RightAngle,
                b'(' => Token::LeftParen,
                b')' => Token::RightParen,
                b'[' => Token::LeftBracket,
                b']' => Token::RightBracket,
                b'{' => Token::LeftBrace,
                b'}' => Token::RightBrace,
                b'+' => Token::Plus,
                b'-' => Token::Minus,
                b'*' => Token::Star,
                b'/' => Token::Slash,
                b'\\' => Token::Backslash,
                b'%' => Token::Hole,
                b'=' => Token::Equals,
                b'^' => Token::Up,
                b'|' => Token::Bar,
                _ => return None,
            };
            (1, token)
        };

        self.advance_ascii(length);
        Some(self.lexeme(token, start))
    }

    fn scan_operator(&mut self, start: Cursor) -> Result<Spanned<Token>, LexError> {
        let content_start = self.cursor.offset + 1;
        let mut end = content_start;
        while let Some(byte) = self.byte_at(end) {
            if byte == b'\'' {
                let operator = self.source[content_start..end].to_owned();
                self.advance_to(end + 1);
                return Ok(self.lexeme(Token::Operator(operator), start));
            }
            if byte == b'\n' {
                break;
            }
            end += 1;
        }

        self.advance_ascii(1);
        Err(self.error(LexErrorKind::MalformedToken, start))
    }

    fn scan_text(&mut self, start: Cursor) -> Result<Spanned<Token>, LexError> {
        self.advance_ascii(1);
        let mut bytes = Vec::new();
        loop {
            let Some(byte) = self.current_byte() else {
                return Err(self.error(LexErrorKind::UnclosedTextLiteral, start));
            };
            match byte {
                b'"' => {
                    self.advance_ascii(1);
                    let text = String::from_utf8(bytes)
                        .map_err(|_| self.error(LexErrorKind::InvalidTextEncoding, start))?;
                    return Ok(self.lexeme(Token::TextLiteral(text), start));
                }
                b'\n' => {
                    self.advance_ascii(1);
                    return Err(self.error(LexErrorKind::UnclosedTextLiteral, start));
                }
                0x00..=0x1f | 0x7f => {
                    self.advance_ascii(1);
                    return Err(self.error(LexErrorKind::IllegalControlCharacter, start));
                }
                b'\\' => self.scan_escape(start, &mut bytes)?,
                0x20..=0x7e => {
                    bytes.push(byte);
                    self.advance_ascii(1);
                }
                _ => {
                    let character = self.source[self.cursor.offset..]
                        .chars()
                        .next()
                        .expect("non-ASCII byte begins a source character");
                    let end = self.cursor.offset + character.len_utf8();
                    bytes.extend_from_slice(&self.source.as_bytes()[self.cursor.offset..end]);
                    self.advance_to(end);
                }
            }
        }
    }

    fn scan_escape(&mut self, start: Cursor, bytes: &mut Vec<u8>) -> Result<(), LexError> {
        let escape_start = self.cursor.offset;
        let Some(escape) = self.byte_at(escape_start + 1) else {
            self.cursor = Cursor {
                offset: start.offset + 1,
                ..start
            };
            return Err(self.error(LexErrorKind::MalformedToken, start));
        };

        let simple = match escape {
            b'n' => Some(b'\n'),
            b'r' => Some(b'\r'),
            b't' => Some(b'\t'),
            b'\\' => Some(b'\\'),
            b'\'' => Some(b'\''),
            b'"' => Some(b'"'),
            _ => None,
        };
        if let Some(byte) = simple {
            bytes.push(byte);
            self.advance_ascii(2);
            return Ok(());
        }

        if is_hex_digit(escape) && self.byte_at(escape_start + 2).is_some_and(is_hex_digit) {
            let digits = &self.source[escape_start + 1..escape_start + 3];
            bytes.push(u8::from_str_radix(digits, 16).expect("two hexadecimal digits"));
            self.advance_ascii(3);
            return Ok(());
        }

        if escape == b'u' && self.byte_at(escape_start + 2) == Some(b'{') {
            let digits_start = escape_start + 3;
            if self.byte_at(digits_start).is_some_and(is_hex_digit) {
                let digits_end = self.separated_digits_end(digits_start, is_hex_digit);
                if self.byte_at(digits_end) == Some(b'}') {
                    let digits = without_underscores(&self.source[digits_start..digits_end]);
                    self.advance_to(digits_end + 1);
                    let codepoint = i64::from_str_radix(&digits, 16)
                        .map_err(|_| self.error(LexErrorKind::InvalidUnicodeEscape, start))?;
                    let encoded = utf8::encode(&[codepoint])
                        .map_err(|_| self.error(LexErrorKind::InvalidUnicodeEscape, start))?;
                    bytes.extend(encoded);
                    return Ok(());
                }
            }
        }

        let invalid_end = escape_start
            + 1
            + self.source[escape_start + 1..]
                .chars()
                .next()
                .expect("escape byte exists")
                .len_utf8();
        self.advance_to(invalid_end);
        let position = self.cursor;
        Err(self.error(LexErrorKind::IllegalEscape, position))
    }

    fn skip_block_comment(&mut self, start: Cursor) -> Result<(), LexError> {
        let mut depth = 1usize;
        while depth > 0 {
            if self.is_eof() {
                return Err(self.error(LexErrorKind::UnclosedComment, start));
            }
            if self.starts_with("(;") {
                depth += 1;
                self.advance_ascii(2);
            } else if self.starts_with(";)") {
                depth -= 1;
                self.advance_ascii(2);
            } else if self.current_byte() == Some(b'\n') {
                self.advance_newline();
            } else {
                let character = self.source[self.cursor.offset..]
                    .chars()
                    .next()
                    .expect("nonempty comment source");
                self.advance_ascii(character.len_utf8());
            }
        }
        Ok(())
    }

    fn unrecognized_character(&mut self, start: Cursor) -> LexError {
        let byte = self.current_byte().expect("not at end of input");
        let kind = if byte <= 0x1f || byte == 0x7f {
            self.advance_ascii(1);
            LexErrorKind::MisplacedControlCharacter
        } else if byte.is_ascii() {
            self.advance_ascii(1);
            LexErrorKind::MalformedToken
        } else {
            let character = self.source[self.cursor.offset..]
                .chars()
                .next()
                .expect("non-ASCII byte begins a source character");
            self.advance_ascii(character.len_utf8());
            LexErrorKind::MisplacedUnicodeCharacter
        };
        self.error(kind, start)
    }

    fn identifier_end(&self, start: usize) -> usize {
        let mut end = start;
        while self.byte_at(end).is_some_and(is_identifier_char) {
            end += 1;
        }
        end
    }

    fn separated_digits_end(&self, start: usize, is_valid: fn(u8) -> bool) -> usize {
        let mut end = start + 1;
        while self.byte_at(end).is_some_and(is_valid) {
            end += 1;
        }
        while self.byte_at(end) == Some(b'_') && self.byte_at(end + 1).is_some_and(is_valid) {
            end += 2;
            while self.byte_at(end).is_some_and(is_valid) {
                end += 1;
            }
        }
        end
    }

    fn indentation_end(&self) -> usize {
        let mut end = self.cursor.offset;
        while matches!(self.byte_at(end), Some(b' ' | b'\t')) {
            end += 1;
        }
        end
    }

    fn newline_or_eof_from(&self, start: usize) -> usize {
        self.source.as_bytes()[start..]
            .iter()
            .position(|byte| *byte == b'\n')
            .map_or(self.source.len(), |relative| start + relative)
    }

    fn advance_to_newline_or_eof(&mut self) {
        self.cursor.offset = self.newline_or_eof_from(self.cursor.offset);
    }

    fn advance_to(&mut self, offset: usize) {
        self.cursor.offset = offset;
    }

    fn advance_ascii(&mut self, length: usize) {
        self.cursor.offset += length;
    }

    fn advance_newline(&mut self) {
        self.cursor.offset += 1;
        self.cursor.line += 1;
        self.cursor.line_start = self.cursor.offset;
    }

    fn current_byte(&self) -> Option<u8> {
        self.byte_at(self.cursor.offset)
    }

    fn byte_at(&self, offset: usize) -> Option<u8> {
        self.source.as_bytes().get(offset).copied()
    }

    fn starts_with(&self, prefix: &str) -> bool {
        self.source[self.cursor.offset..].starts_with(prefix)
    }

    fn is_eof(&self) -> bool {
        self.cursor.offset == self.source.len()
    }

    fn position(&self, cursor: Cursor) -> Position {
        Position::new(
            self.file.clone(),
            cursor.line,
            (cursor.offset - cursor.line_start) as i64,
        )
    }

    fn span(&self, start: Cursor) -> Span {
        Span::new(self.position(start), self.position(self.cursor))
    }

    fn lexeme(&self, token: Token, start: Cursor) -> Spanned<Token> {
        Spanned::new(token, self.span(start))
    }

    fn error(&self, kind: LexErrorKind, start: Cursor) -> LexError {
        LexError {
            kind,
            span: self.span(start),
        }
    }
}

impl<Classify> Iterator for Lexer<'_, Classify>
where
    Classify: FnMut(&str) -> bool,
{
    type Item = Result<Spanned<Token>, LexError>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.finished {
            return None;
        }

        let result = self.next_lexeme();
        if match &result {
            Ok(lexeme) => lexeme.node == Token::Eof,
            Err(_) => true,
        } {
            self.finished = true;
        }
        Some(result)
    }
}

const OCAML_INT_MAX: i64 = (1_i64 << 62) - 1;

fn is_digit(byte: u8) -> bool {
    byte.is_ascii_digit()
}

fn is_hex_digit(byte: u8) -> bool {
    byte.is_ascii_digit() || matches!(byte, b'A'..=b'F')
}

fn is_upper(byte: u8) -> bool {
    byte.is_ascii_uppercase()
}

fn is_identifier_start(byte: u8) -> bool {
    byte.is_ascii_alphabetic() || byte == b'_'
}

fn is_identifier_char(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'\'')
}

fn without_underscores(digits: &str) -> String {
    digits
        .chars()
        .filter(|character| *character != '_')
        .collect()
}

fn parse_natural(digits: &str, radix: u32) -> Natural {
    let integer = BigInt::parse_bytes(digits.as_bytes(), radix).expect("nonempty digit sequence");
    Natural::try_from(integer).expect("digit sequence is non-negative")
}

fn keyword(identifier: &str) -> Option<Token> {
    Some(match identifier {
        "bool" => Token::Bool,
        "nat" => Token::Nat,
        "int" => Token::Int,
        "text" => Token::Text,
        "syntax" => Token::Syntax,
        "extern" => Token::Extern,
        "tbl" => Token::Table,
        "relation" => Token::Relation,
        "rulegroup" => Token::RuleGroup,
        "rule" => Token::Rule,
        "var" => Token::Var,
        "builtin" => Token::Builtin,
        "dec" => Token::Dec,
        "def" => Token::Def,
        "if" => Token::If,
        "otherwise" => Token::Otherwise,
        "debug" => Token::Debug,
        "eps" => Token::Epsilon,
        "true" => Token::BoolLiteral(true),
        "false" => Token::BoolLiteral(false),
        _ => return None,
    })
}
