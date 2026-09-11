//! Stateful STF tokenization with a separate packet-data vocabulary
//!
//! Each iteration first skips layout for the current mode and then dispatches
//! to command or packet-data lexing. Command mode recognizes keywords,
//! identifiers, numbers, and punctuation; `packet` and `expect` switch to
//! packet-data mode until newline or `$`. For example, `**` after `expect`
//! becomes two packet wildcards rather than identifier punctuation.

use std::rc::Rc;

use crate::{
    lang::common::source::{Phrase, Position, Span},
    phrase,
};

use super::error::{StfError, StfErrorKind};

// == Tokens

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Token {
    End,
    Add,
    All,
    Bytes,
    CheckCounter,
    Expect,
    NoPacket,
    Packet,
    Packets,
    Exact,
    Remove,
    SetDefault,
    Wait,
    PacketWildcard,
    DataTernary,
    MirroringAdd,
    MirroringAddMc,
    MirroringGet,
    McGroupCreate,
    McNodeCreate,
    McNodeAssociate,
    RegisterRead,
    RegisterWrite,
    RegisterReset,
    Id(String),
    Colon,
    Comma,
    Dot,
    LeftParen,
    RightParen,
    Slash,
    Assign,
    Eq,
    Lt,
    Le,
    Gt,
    Ge,
    Ne,
    LeftBracket,
    RightBracket,
    IntDecimal(String),
    IntHex(String),
    IntBinary(String),
    TernaryHex(String),
    DataDecimal(String),
    DataHex(String),
}

// == Lexer modes

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Mode {
    Command,
    PacketData,
}

// == Lexer

pub struct Lexer<'source> {
    source: &'source str,
    file: Rc<str>,
    index: usize,
    line: i64,
    column: i64,
    mode: Mode,
    finished: bool,
}

impl<'source> Lexer<'source> {
    // - Construction

    pub fn new(file: impl Into<Rc<str>>, source: &'source str) -> Self {
        Self {
            source,
            file: file.into(),
            index: 0,
            line: 1,
            column: 0,
            mode: Mode::Command,
            finished: false,
        }
    }

    // - Source cursor

    fn source_position(&self) -> Position {
        Position::new(Rc::clone(&self.file), self.line, self.column)
    }

    fn span_from(&self, pos_l: Position) -> Span {
        Span::new(pos_l, self.source_position())
    }

    fn bump(&mut self) -> Option<char> {
        let character = self.source[self.index..].chars().next()?;
        self.index += character.len_utf8();
        if character == '\n' {
            self.line += 1;
            self.column = 0;
        } else {
            self.column += character.len_utf8() as i64;
        }
        Some(character)
    }

    fn peek(&self) -> Option<char> {
        self.source[self.index..].chars().next()
    }

    fn take_while(&mut self, mut predicate: impl FnMut(char) -> bool) -> &'source str {
        let start = self.index;
        while let Some(character) = self.peek() {
            if !predicate(character) {
                break;
            }
            self.bump();
        }
        &self.source[start..self.index]
    }

    fn error(&self, kind: StfErrorKind, pos_l: Position) -> StfError {
        let span = self.span_from(pos_l);
        StfError::new(kind, span)
    }

    // - Layout

    fn skip_layout(&mut self) {
        loop {
            match self.mode {
                Mode::Command => {
                    self.skip_command_layout();
                    return;
                }
                Mode::PacketData => {
                    self.take_while(|character| matches!(character, ' ' | '\t' | '\r'));
                    if self.peek() != Some('\n') {
                        return;
                    }
                    self.take_while(|character| character == '\n');
                    self.mode = Mode::Command;
                }
            }
        }
    }

    fn skip_command_layout(&mut self) {
        loop {
            self.take_while(|character| matches!(character, ' ' | '\t' | '\r' | '\n'));
            if self.peek() != Some('#') {
                break;
            }
            self.take_while(|character| character != '\n');
        }
    }

    // - Command tokens

    fn lex_command(&mut self) -> Result<Token, StfError> {
        let pos_l = self.source_position();
        let Some(character) = self.peek() else {
            self.finished = true;
            return Ok(Token::End);
        };

        if let Some(token) = self.lex_punctuation() {
            return Ok(token);
        }

        if matches!(character, '=' | '!' | '<' | '>') {
            let token = self.lex_operator(character, pos_l)?;
            return Ok(token);
        }

        if character == '"' {
            let token = self.lex_quoted_identifier(pos_l)?;
            return Ok(token);
        }

        if character.is_ascii_digit() {
            let token = self.lex_number(pos_l)?;
            return Ok(token);
        }

        if character == '$' || character == '_' || character.is_ascii_alphabetic() {
            let token = self.lex_identifier();
            return Ok(token);
        }

        self.bump();
        let error = StfErrorKind::InvalidCharacter(character);
        Err(self.error(error, pos_l))
    }

    fn lex_punctuation(&mut self) -> Option<Token> {
        let token = match self.peek()? {
            ':' => Token::Colon,
            ',' => Token::Comma,
            '.' => Token::Dot,
            '[' => Token::LeftBracket,
            ']' => Token::RightBracket,
            '(' => Token::LeftParen,
            ')' => Token::RightParen,
            '/' => Token::Slash,
            _ => return None,
        };
        self.bump();
        Some(token)
    }

    fn lex_operator(&mut self, character: char, pos_l: Position) -> Result<Token, StfError> {
        self.bump();
        let paired = self.peek() == Some('=');
        if paired {
            self.bump();
        }
        match (character, paired) {
            ('=', false) => Ok(Token::Assign),
            ('=', true) => Ok(Token::Eq),
            ('!', true) => Ok(Token::Ne),
            ('<', false) => Ok(Token::Lt),
            ('<', true) => Ok(Token::Le),
            ('>', false) => Ok(Token::Gt),
            ('>', true) => Ok(Token::Ge),
            _ => {
                let error = StfErrorKind::InvalidCharacter(character);
                Err(self.error(error, pos_l))
            }
        }
    }

    fn lex_quoted_identifier(&mut self, pos_l: Position) -> Result<Token, StfError> {
        self.bump();
        let start = self.index;
        self.take_while(|character| character != '"' && character != '\n');
        if self.peek() != Some('"') {
            let error = StfErrorKind::UnterminatedQuotedIdentifier;
            return Err(self.error(error, pos_l));
        }
        let id = self.source[start..self.index].to_owned();
        self.bump();
        Ok(Token::Id(id))
    }

    fn lex_number(&mut self, pos_l: Position) -> Result<Token, StfError> {
        let spelling =
            self.take_while(|character| character.is_ascii_alphanumeric() || character == '*');
        let digits = spelling.get(2..).unwrap_or_default();
        let valid = if spelling.starts_with("0x") || spelling.starts_with("0X") {
            !digits.is_empty()
                && digits
                    .chars()
                    .all(|digit| digit.is_ascii_hexdigit() || digit == '*')
        } else if spelling.starts_with("0b") || spelling.starts_with("0B") {
            !digits.is_empty() && digits.chars().all(|digit| matches!(digit, '0' | '1' | '*'))
        } else {
            spelling.chars().all(|digit| digit.is_ascii_digit())
        };
        if !valid {
            let error = StfErrorKind::InvalidNumber(spelling.to_owned());
            return Err(self.error(error, pos_l));
        }

        let token = if spelling.starts_with("0x") || spelling.starts_with("0X") {
            if spelling.contains('*') {
                Token::TernaryHex(spelling.to_owned())
            } else {
                Token::IntHex(spelling.to_owned())
            }
        } else if spelling.starts_with("0b") || spelling.starts_with("0B") {
            Token::IntBinary(spelling.to_owned())
        } else {
            Token::IntDecimal(spelling.to_owned())
        };
        Ok(token)
    }

    fn lex_identifier(&mut self) -> Token {
        let id = self.take_while(|character| {
            character == '$' || character == '_' || character.is_ascii_alphanumeric()
        });
        self.classify_identifier(id)
    }

    fn classify_identifier(&mut self, id: &str) -> Token {
        match id {
            "add" => Token::Add,
            "all" => Token::All,
            "bytes" => Token::Bytes,
            "check_counter" => Token::CheckCounter,
            "expect" => {
                self.mode = Mode::PacketData;
                Token::Expect
            }
            "no_packet" => Token::NoPacket,
            "packet" => {
                self.mode = Mode::PacketData;
                Token::Packet
            }
            "packets" => Token::Packets,
            "remove" => Token::Remove,
            "setdefault" => Token::SetDefault,
            "wait" => Token::Wait,
            "mirroring_add" => Token::MirroringAdd,
            "mirroring_add_mc" => Token::MirroringAddMc,
            "mirroring_get" => Token::MirroringGet,
            "mc_mgrp_create" => Token::McGroupCreate,
            "mc_node_create" => Token::McNodeCreate,
            "mc_node_associate" => Token::McNodeAssociate,
            "register_read" => Token::RegisterRead,
            "register_write" => Token::RegisterWrite,
            "register_reset" => Token::RegisterReset,
            _ => Token::Id(id.to_owned()),
        }
    }

    // - Packet-data tokens

    fn lex_packet_data(&mut self) -> Result<Token, StfError> {
        let pos_l = self.source_position();
        let Some(character) = self.peek() else {
            self.mode = Mode::Command;
            self.finished = true;
            return Ok(Token::End);
        };
        if character == '$' {
            self.bump();
            self.mode = Mode::Command;
            return Ok(Token::Exact);
        }
        if character == '*' {
            self.bump();
            return Ok(Token::PacketWildcard);
        }
        if character == '?' {
            self.take_while(|next| next == '?');
            return Ok(Token::DataTernary);
        }
        if character.is_ascii_hexdigit() {
            let spelling = self.take_while(|next| next.is_ascii_hexdigit()).to_owned();
            let token = if spelling.chars().all(|next| next.is_ascii_digit()) {
                Token::DataDecimal(spelling)
            } else {
                Token::DataHex(spelling)
            };
            return Ok(token);
        }
        self.bump();
        let error = StfErrorKind::InvalidCharacter(character);
        Err(self.error(error, pos_l))
    }
}

// == Token stream

impl Iterator for Lexer<'_> {
    type Item = Result<Phrase<Token>, StfError>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.finished {
            return None;
        }
        self.skip_layout();
        let pos_l = self.source_position();
        let token = match self.mode {
            Mode::Command => self.lex_command(),
            Mode::PacketData => self.lex_packet_data(),
        };
        let pos_r = self.source_position();
        let span = Span::new(pos_l, pos_r);
        Some(token.map(|node| phrase! { node: node, span: span }))
    }
}
