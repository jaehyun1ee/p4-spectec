//! Lazy source-level tokenization for SpecTec
//!
//! `Iterator::next` requests one lexeme from `Lexer::scan_token`, the main
//! lexer state. It dispatches on the current byte and transitions to
//! `scan_after_newline`, `scan_after_two_newlines`, `scan_comment`, or
//! `scan_text` when those inputs need their own state. Each state advances the
//! UTF-8 byte cursor and attaches the traversed [`Span`] to its resulting
//! [`Token`].
//!
//! Branches with a shared prefix implement maximal munch locally: comments
//! precede punctuation, comma-newline precedes comma, and specialized tag,
//! dot-identifier, and numbered-hole rules precede their shorter fallbacks.
//! `scan_fixed` likewise checks longer punctuation before its prefixes.
//! `scan_identifier` recognizes keywords and asks the parser-owned classifier
//! whether an uppercase name is a variable, while `scan_text` delegates escapes
//! to `scan_escape`.
//!
//! The downstream `tokens::parser_tokens` adapter inserts implicit `Sequence`
//! tokens, distinguishes postfix iteration from arithmetic multiplication, and
//! interns source positions for LALRPOP.
//!
//! Source and decoded text literals are UTF-8 strings. Hex byte escapes may
//! combine into a valid UTF-8 sequence; byte-only results are rejected with
//! [`LexErrorKind::InvalidTextEncoding`] so tokens fit the language model's
//! `String` text representation.
//!
//! # Examples
//!
//! ```text
//! source: F(x)
//! lexer:  UpperIdLeftParen("F"), LowerId("x"), RightParen
//!
//! source: A B
//! lexer:  UpperId("A"), UpperId("B")
//! parser: UpperId("A"), Sequence, UpperId("B")
//!
//! source: X                  classifier("X") = variable
//! lexer:  LowerId("X")
//!
//! source: "\48\69"
//! lexer:  TextLiteral("Hi")
//! ```

use std::rc::Rc;

use num_bigint::BigInt;

use crate::lang::{
    common::source::{Phrase, Position, Span},
    xl::{num::Natural, utf8},
};

use super::error::{LexError, LexErrorKind};

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
    /// Parser-only marker for juxtaposed grammar atoms
    Sequence,
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
    /// Parser-only spelling of `*` when it closes an iterated expression
    IterStar,
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

/// A byte cursor into a UTF-8 source string
#[derive(Clone, Copy)]
struct Cursor {
    offset: usize,
    line: i64,
    line_start: usize,
}

/// A lazy SpecTec token stream
///
/// The classifier relabels uppercase identifiers that are variables in the
/// parser's current scope. Input is valid UTF-8 by construction; file entry
/// points must report decoding failures before constructing a lexer.
pub struct Lexer<'input, Classify> {
    file: Rc<str>,
    source: &'input str,
    cursor: Cursor,
    finished: bool,
    classify_uppercase: Classify,
}

// - Construction

impl<'input, Classify> Lexer<'input, Classify>
where
    Classify: FnMut(&str) -> bool,
{
    /// Tokenizes `source` using the parser's uppercase-variable classifier
    pub fn new(
        file: impl Into<Rc<str>>,
        source: &'input str,
        classify_uppercase: Classify,
    ) -> Self {
        Self {
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
    // - Cursor movement

    fn advance_to(&mut self, offset: usize) {
        self.cursor.offset = offset;
    }

    fn advance_add(&mut self, length: usize) {
        self.cursor.offset += length;
    }

    fn advance_newline(&mut self) {
        self.cursor.offset += 1;
        self.cursor.line += 1;
        self.cursor.line_start = self.cursor.offset;
    }

    fn advance_to_line_end(&mut self) {
        self.cursor.offset = self.find_line_end(self.cursor.offset);
    }

    // - Cursor inspection

    fn cursor_current(&self) -> Option<u8> {
        self.cursor_offset(self.cursor.offset)
    }

    fn cursor_offset(&self, offset: usize) -> Option<u8> {
        self.source.as_bytes().get(offset).copied()
    }

    fn cursor_starts_with(&self, prefix: &str) -> bool {
        self.source[self.cursor.offset..].starts_with(prefix)
    }

    fn cursor_is_eof(&self) -> bool {
        self.cursor.offset == self.source.len()
    }

    // - Finders for token boundaries

    fn find_indentation_end(&self) -> usize {
        let mut end = self.cursor.offset;
        while matches!(self.cursor_offset(end), Some(b' ' | b'\t')) {
            end += 1;
        }
        end
    }

    fn find_line_end(&self, start: usize) -> usize {
        self.source.as_bytes()[start..]
            .iter()
            .position(|byte| *byte == b'\n')
            .map_or(self.source.len(), |relative| start + relative)
    }

    fn find_identifier_end(&self, start: usize) -> usize {
        let mut end = start;
        while self
            .cursor_offset(end)
            .is_some_and(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'\''))
        {
            end += 1;
        }
        end
    }

    fn find_separated_digits_end(&self, start: usize, is_valid: fn(u8) -> bool) -> usize {
        let mut end = start + 1;
        while self.cursor_offset(end).is_some_and(is_valid) {
            end += 1;
        }
        while self.cursor_offset(end) == Some(b'_')
            && self.cursor_offset(end + 1).is_some_and(is_valid)
        {
            end += 2;
            while self.cursor_offset(end).is_some_and(is_valid) {
                end += 1;
            }
        }
        end
    }

    // - Source locations and results

    fn position(&self, cursor: Cursor) -> Position {
        Position::new(
            self.file.clone(),
            cursor.line,
            (cursor.offset - cursor.line_start) as i64,
        )
    }

    fn span(&self, cursor_start: Cursor) -> Span {
        Span::new(self.position(cursor_start), self.position(self.cursor))
    }

    fn lexeme(&self, token: Token, start: Cursor) -> Phrase<Token> {
        crate::phrase! {
            node: token,
            span: self.span(start),
        }
    }

    // - Helpers

    fn is_identifier_start(byte: u8) -> bool {
        byte.is_ascii_alphabetic() || byte == b'_'
    }

    fn is_digit(byte: u8) -> bool {
        byte.is_ascii_digit()
    }

    fn is_hex_digit(byte: u8) -> bool {
        byte.is_ascii_digit() || matches!(byte, b'A'..=b'F')
    }

    fn strip_underscores(digits: &str) -> String {
        digits
            .chars()
            .filter(|character| *character != '_')
            .collect()
    }

    // - Token state

    fn scan_token(&mut self) -> Result<Phrase<Token>, LexError> {
        loop {
            let start = self.cursor;
            if self.cursor_is_eof() {
                return Ok(self.lexeme(Token::Eof, start));
            }

            let byte = self.cursor_current().expect("cursor is within source");
            match byte {
                b'(' if self.cursor_starts_with("(;") => {
                    self.advance_add(2);
                    self.scan_comment(start)?;
                    continue;
                }
                b';' if self.cursor_starts_with(";;") => {
                    if let Some(lexeme) = self.scan_line_comment(start)? {
                        return Ok(lexeme);
                    }
                    continue;
                }
                b'\\' if self.cursor_starts_with("\\\n") => {
                    self.advance_add(1);
                    self.advance_newline();
                    continue;
                }
                b'\n' => {
                    self.advance_newline();
                    if let Some(lexeme) = self.scan_after_newline()? {
                        return Ok(lexeme);
                    }
                    continue;
                }
                b' ' | b'\t' | b'\r' => {
                    self.advance_add(1);
                    continue;
                }
                b'"' => return self.scan_text(start),
                b'\'' => return self.scan_operator(start),
                b',' => {
                    if let Some(lexeme) = self.scan_comma_newline(start) {
                        return Ok(lexeme);
                    }
                }
                b'_' => {
                    if let Some(lexeme) = self.scan_tag(start) {
                        return Ok(lexeme);
                    }
                }
                b'.' => {
                    if let Some(lexeme) = self.scan_dot_identifier(start) {
                        return Ok(lexeme);
                    }
                }
                b'%' => {
                    if let Some(lexeme) = self.scan_numbered_hole(start)? {
                        return Ok(lexeme);
                    }
                }
                _ => {}
            }

            if Self::is_digit(byte) {
                return Ok(self.scan_number(start).expect("digit starts a number"));
            }
            if Self::is_identifier_start(byte) {
                return Ok(self
                    .scan_identifier(start)
                    .expect("identifier-start byte starts an identifier"));
            }
            if let Some(lexeme) = self.scan_fixed(start) {
                return Ok(lexeme);
            }

            return Err(self.unrecognized_character(start));
        }
    }

    // - Newline states

    fn scan_after_newline(&mut self) -> Result<Option<Phrase<Token>>, LexError> {
        if let Some(lexeme) = self.scan_newline_bar() {
            return Ok(Some(lexeme));
        }

        let end = self.find_indentation_end();
        if self.cursor_offset(end) == Some(b'\n') {
            self.advance_to(end);
            self.advance_newline();
            return self.scan_after_two_newlines().map(Some);
        }

        Ok(None)
    }

    fn scan_after_two_newlines(&mut self) -> Result<Phrase<Token>, LexError> {
        loop {
            if let Some(lexeme) = self.scan_newline_bar() {
                return Ok(lexeme);
            }

            let start = self.cursor;
            let indent_end = self.find_indentation_end();
            if self.cursor_offset(indent_end) == Some(b'\n') {
                self.advance_to(indent_end);
                self.advance_newline();
                return Ok(self.lexeme(Token::Newline3, start));
            }

            if self.source[indent_end..].starts_with(";;") {
                let line_end = self.find_line_end(indent_end);
                if self.cursor_offset(line_end) == Some(b'\n') {
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

    fn scan_newline_bar(&mut self) -> Option<Phrase<Token>> {
        let start = self.cursor;
        let indent_end = self.find_indentation_end();
        if self.cursor_offset(indent_end) != Some(b'|')
            || !matches!(self.cursor_offset(indent_end + 1), Some(b' ' | b'\t'))
        {
            return None;
        }

        self.advance_to(indent_end + 2);
        Some(self.lexeme(Token::NewlineBar, start))
    }

    // - Comment state

    fn scan_comment(&mut self, start: Cursor) -> Result<(), LexError> {
        let mut depth = 1usize;
        while depth > 0 {
            if self.cursor_is_eof() {
                return Err(self.error(LexErrorKind::UnclosedComment, start));
            }
            if self.cursor_starts_with("(;") {
                depth += 1;
                self.advance_add(2);
            } else if self.cursor_starts_with(";)") {
                depth -= 1;
                self.advance_add(2);
            } else if self.cursor_current() == Some(b'\n') {
                self.advance_newline();
            } else {
                let character = self.source[self.cursor.offset..]
                    .chars()
                    .next()
                    .expect("nonempty comment source");
                self.advance_add(character.len_utf8());
            }
        }
        Ok(())
    }

    // - Token-state layout rules

    fn scan_line_comment(&mut self, start: Cursor) -> Result<Option<Phrase<Token>>, LexError> {
        self.advance_to_line_end();
        if self.cursor_is_eof() {
            return Ok(Some(self.lexeme(Token::Eof, start)));
        }

        self.advance_newline();
        self.scan_after_newline()
    }

    fn scan_comma_newline(&mut self, start: Cursor) -> Option<Phrase<Token>> {
        if self.cursor_current() != Some(b',') {
            return None;
        }

        let mut end = self.cursor.offset + 1;
        while matches!(self.cursor_offset(end), Some(b' ' | b'\t')) {
            end += 1;
        }
        if self.source[end..].starts_with(";;") {
            end = self.find_line_end(end);
        }
        if self.cursor_offset(end) != Some(b'\n') {
            return None;
        }

        self.advance_to(end);
        self.advance_newline();
        Some(self.lexeme(Token::CommaNewline, start))
    }

    // - Token-state identifier rules

    fn scan_tag(&mut self, start: Cursor) -> Option<Phrase<Token>> {
        if self.cursor_current() != Some(b'_')
            || !self
                .cursor_offset(self.cursor.offset + 1)?
                .is_ascii_uppercase()
        {
            return None;
        }

        let id_start = self.cursor.offset + 1;
        let end = self.find_identifier_end(id_start);
        if matches!(self.cursor_offset(end), Some(b'(' | b'<')) {
            return None;
        }
        let identifier = self.source[id_start..end].to_owned();
        self.advance_to(end);
        Some(self.lexeme(Token::TagUpperId(identifier), start))
    }

    fn scan_dot_identifier(&mut self, start: Cursor) -> Option<Phrase<Token>> {
        if self.cursor_current() != Some(b'.')
            || !Self::is_identifier_start(self.cursor_offset(self.cursor.offset + 1)?)
        {
            return None;
        }

        let id_start = self.cursor.offset + 1;
        let end = self.find_identifier_end(id_start);
        if end - start.offset <= 3 && self.source[start.offset..].starts_with("...") {
            return None;
        }

        let identifier = self.source[id_start..end].to_owned();
        self.advance_to(end);
        Some(self.lexeme(Token::DotId(identifier), start))
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

    fn scan_identifier(&mut self, start: Cursor) -> Option<Phrase<Token>> {
        let first = self.cursor_current()?;
        if !Self::is_identifier_start(first) {
            return None;
        }

        let is_uppercase = first.is_ascii_uppercase();
        let end = self.find_identifier_end(self.cursor.offset);
        let identifier = self.source[self.cursor.offset..end].to_owned();
        let suffix = self.cursor_offset(end);
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
                if let Some(keyword) = Self::keyword(&identifier) {
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

    // - Token-state numbered holes

    fn scan_numbered_hole(&mut self, start: Cursor) -> Result<Option<Phrase<Token>>, LexError> {
        if self.cursor_current() != Some(b'%')
            || !Self::is_digit(
                self.cursor_offset(self.cursor.offset + 1)
                    .unwrap_or_default(),
            )
        {
            return Ok(None);
        }

        let end = self.find_separated_digits_end(self.cursor.offset + 1, Self::is_digit);
        let digits = Self::strip_underscores(&self.source[self.cursor.offset + 1..end]);
        self.advance_to(end);
        let number = digits
            .parse::<i64>()
            .map_err(|_| self.error(LexErrorKind::HoleNumberOutOfRange, start))?;
        Ok(Some(self.lexeme(Token::NumberedHole(number), start)))
    }

    // - Token-state Numbers

    fn parse_natural(digits: &str, radix: u32) -> Natural {
        let integer =
            BigInt::parse_bytes(digits.as_bytes(), radix).expect("nonempty digit sequence");
        Natural::try_from(integer).expect("digit sequence is non-negative")
    }

    fn scan_number(&mut self, start: Cursor) -> Option<Phrase<Token>> {
        if !Self::is_digit(self.cursor_current()?) {
            return None;
        }

        if self.cursor_starts_with("0x")
            && self
                .cursor_offset(self.cursor.offset + 2)
                .is_some_and(Self::is_hex_digit)
        {
            let end = self.find_separated_digits_end(self.cursor.offset + 2, Self::is_hex_digit);
            let digits = Self::strip_underscores(&self.source[self.cursor.offset + 2..end]);
            let natural = Self::parse_natural(&digits, 16);
            self.advance_to(end);
            return Some(self.lexeme(Token::HexLiteral(natural), start));
        }

        let end = self.find_separated_digits_end(self.cursor.offset, Self::is_digit);
        let digits = Self::strip_underscores(&self.source[self.cursor.offset..end]);
        let natural = Self::parse_natural(&digits, 10);
        self.advance_to(end);
        Some(self.lexeme(Token::NaturalLiteral(natural), start))
    }

    // - Token-state fixed rules

    fn scan_fixed(&mut self, start: Cursor) -> Option<Phrase<Token>> {
        let (length, token) = if self.cursor_starts_with("->_") {
            (3, Token::ArrowSub)
        } else if self.cursor_starts_with("=>_") {
            (3, Token::DoubleArrowSub)
        } else if self.cursor_starts_with("<=>") {
            (3, Token::DoubleArrowBoth)
        } else if self.cursor_starts_with("==>") {
            (3, Token::DoubleArrowLong)
        } else if self.cursor_starts_with("~>*") {
            (3, Token::SquigglyArrowStar)
        } else if self.cursor_starts_with("=/=") {
            (3, Token::NotEquals)
        } else if self.cursor_starts_with("%latex") {
            (6, Token::Latex)
        } else if self.cursor_starts_with("`(") {
            (2, Token::TickLeftParen)
        } else if self.cursor_starts_with("`)") {
            (2, Token::TickRightParen)
        } else if self.cursor_starts_with("`[") {
            (2, Token::TickLeftBracket)
        } else if self.cursor_starts_with("`]") {
            (2, Token::TickRightBracket)
        } else if self.cursor_starts_with("`{") {
            (2, Token::TickLeftBrace)
        } else if self.cursor_starts_with("`}") {
            (2, Token::TickRightBrace)
        } else if self.cursor_starts_with("`<") {
            (2, Token::TickLeftAngle)
        } else if self.cursor_starts_with("`>") {
            (2, Token::TickRightAngle)
        } else if self.cursor_starts_with("|-") {
            (2, Token::Turnstile)
        } else if self.cursor_starts_with("-|") {
            (2, Token::Tilesturn)
        } else if self.cursor_starts_with("->") {
            (2, Token::Arrow)
        } else if self.cursor_starts_with("=>") {
            (2, Token::DoubleArrow)
        } else if self.cursor_starts_with("~>") {
            (2, Token::SquigglyArrow)
        } else if self.cursor_starts_with("/\\") {
            (2, Token::And)
        } else if self.cursor_starts_with("\\/") {
            (2, Token::Or)
        } else if self.cursor_starts_with("...") {
            (3, Token::TripleDot)
        } else if self.cursor_starts_with("..") {
            (2, Token::DoubleDot)
        } else if self.cursor_starts_with("::") {
            (2, Token::DoubleColon)
        } else if self.cursor_starts_with(":/") {
            (2, Token::ColonSlash)
        } else if self.cursor_starts_with(":=") {
            (2, Token::ColonEquals)
        } else if self.cursor_starts_with("##") {
            (2, Token::DoubleHash)
        } else if self.cursor_starts_with("<:") {
            (2, Token::Subtype)
        } else if self.cursor_starts_with("~~") {
            (2, Token::DoubleTilde)
        } else if self.cursor_starts_with("<-") {
            (2, Token::LeftAngleDash)
        } else if self.cursor_starts_with("<=") {
            (2, Token::LeftAngleEquals)
        } else if self.cursor_starts_with(">=") {
            (2, Token::RightAngleEquals)
        } else if self.cursor_starts_with(">(") {
            (2, Token::RightAngleLeftParen)
        } else if self.cursor_starts_with("++") {
            (2, Token::DoublePlus)
        } else if self.cursor_starts_with("--") {
            (2, Token::Dash)
        } else if self.cursor_starts_with("%%") {
            (2, Token::MultipleHole)
        } else if self.cursor_starts_with("!%") {
            (2, Token::EmptyHole)
        } else {
            let token = match self.cursor_current()? {
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

        self.advance_add(length);
        Some(self.lexeme(token, start))
    }

    // - Token-state operator rule

    fn scan_operator(&mut self, start: Cursor) -> Result<Phrase<Token>, LexError> {
        let content_start = self.cursor.offset + 1;
        let mut end = content_start;
        while let Some(byte) = self.cursor_offset(end) {
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

        self.advance_add(1);
        Err(self.error(LexErrorKind::MalformedToken, start))
    }

    // - Text state

    fn scan_text(&mut self, start: Cursor) -> Result<Phrase<Token>, LexError> {
        self.advance_add(1);
        let mut bytes = Vec::new();
        loop {
            let Some(byte) = self.cursor_current() else {
                return Err(self.error(LexErrorKind::UnclosedTextLiteral, start));
            };
            match byte {
                b'"' => {
                    self.advance_add(1);
                    let text = String::from_utf8(bytes)
                        .map_err(|_| self.error(LexErrorKind::InvalidTextEncoding, start))?;
                    return Ok(self.lexeme(Token::TextLiteral(text), start));
                }
                b'\n' => {
                    self.advance_add(1);
                    return Err(self.error(LexErrorKind::UnclosedTextLiteral, start));
                }
                0x00..=0x1f | 0x7f => {
                    self.advance_add(1);
                    return Err(self.error(LexErrorKind::IllegalControlCharacter, start));
                }
                b'\\' => self.scan_escape(start, &mut bytes)?,
                0x20..=0x7e => {
                    bytes.push(byte);
                    self.advance_add(1);
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
        let Some(escape) = self.cursor_offset(escape_start + 1) else {
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
            self.advance_add(2);
            return Ok(());
        }

        if Self::is_hex_digit(escape)
            && self
                .cursor_offset(escape_start + 2)
                .is_some_and(Self::is_hex_digit)
        {
            let digits = &self.source[escape_start + 1..escape_start + 3];
            bytes.push(u8::from_str_radix(digits, 16).expect("two hexadecimal digits"));
            self.advance_add(3);
            return Ok(());
        }

        if escape == b'u' && self.cursor_offset(escape_start + 2) == Some(b'{') {
            let digits_start = escape_start + 3;
            if self
                .cursor_offset(digits_start)
                .is_some_and(Self::is_hex_digit)
            {
                let digits_end = self.find_separated_digits_end(digits_start, Self::is_hex_digit);
                if self.cursor_offset(digits_end) == Some(b'}') {
                    let digits = Self::strip_underscores(&self.source[digits_start..digits_end]);
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

    // - Errors

    fn unrecognized_character(&mut self, start: Cursor) -> LexError {
        let byte = self.cursor_current().expect("not at end of input");
        let kind = if byte <= 0x1f || byte == 0x7f {
            self.advance_add(1);
            LexErrorKind::MisplacedControlCharacter
        } else if byte.is_ascii() {
            self.advance_add(1);
            LexErrorKind::MalformedToken
        } else {
            let character = self.source[self.cursor.offset..]
                .chars()
                .next()
                .expect("non-ASCII byte begins a source character");
            self.advance_add(character.len_utf8());
            LexErrorKind::MisplacedUnicodeCharacter
        };
        self.error(kind, start)
    }

    fn error(&self, kind: LexErrorKind, start: Cursor) -> LexError {
        crate::phrase! {
            node: kind,
            span: self.span(start),
        }
    }
}

impl<Classify> Iterator for Lexer<'_, Classify>
where
    Classify: FnMut(&str) -> bool,
{
    type Item = Result<Phrase<Token>, LexError>;

    // - Iteration

    fn next(&mut self) -> Option<Self::Item> {
        if self.finished {
            return None;
        }

        let result = self.scan_token();
        if match &result {
            Ok(lexeme) => lexeme.node == Token::Eof,
            Err(_) => true,
        } {
            self.finished = true;
        }
        Some(result)
    }
}
