//! Stateful STF lexer. Packet payloads deliberately use a different token vocabulary.

use std::rc::Rc;

use crate::lang::common::source::{Position, Span};

use super::error::{StfError, StfErrorKind};

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

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Mode {
    Keyword,
    PacketData,
}

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
    pub fn new(file: impl Into<Rc<str>>, source: &'source str) -> Self {
        Self {
            source,
            file: file.into(),
            index: 0,
            line: 1,
            column: 0,
            mode: Mode::Keyword,
            finished: false,
        }
    }

    fn source_position(&self) -> Position {
        Position::new(Rc::clone(&self.file), self.line, self.column)
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

    fn take_while(&mut self, predicate: impl Fn(char) -> bool) -> &'source str {
        let start = self.index;
        while self.peek().is_some_and(&predicate) {
            self.bump();
        }
        &self.source[start..self.index]
    }

    fn error(&self, kind: StfErrorKind, left: Position) -> StfError {
        StfError::new(kind, Span::new(left, self.source_position()))
    }

    fn skip_keyword_layout(&mut self) {
        loop {
            self.take_while(|character| matches!(character, ' ' | '\t' | '\r' | '\n'));
            if self.peek() != Some('#') {
                break;
            }
            self.take_while(|character| character != '\n');
        }
    }

    fn identifier(&mut self) -> String {
        self.take_while(|character| {
            character == '$'
                || character == '_'
                || character == '.'
                || character.is_ascii_alphanumeric()
        })
        .to_owned()
    }

    fn keyword_token(&mut self) -> Result<Token, StfError> {
        let left = self.source_position();
        let Some(character) = self.peek() else {
            self.finished = true;
            return Ok(Token::End);
        };

        let punctuation = match character {
            ':' => Some(Token::Colon),
            ',' => Some(Token::Comma),
            '.' => Some(Token::Dot),
            '[' => Some(Token::LeftBracket),
            ']' => Some(Token::RightBracket),
            '(' => Some(Token::LeftParen),
            ')' => Some(Token::RightParen),
            '/' => Some(Token::Slash),
            _ => None,
        };
        if let Some(token) = punctuation {
            self.bump();
            return Ok(token);
        }

        if matches!(character, '=' | '!' | '<' | '>') {
            self.bump();
            let paired = self.peek() == Some('=');
            if paired {
                self.bump();
            }
            return match (character, paired) {
                ('=', false) => Ok(Token::Assign),
                ('=', true) => Ok(Token::Eq),
                ('!', true) => Ok(Token::Ne),
                ('<', false) => Ok(Token::Lt),
                ('<', true) => Ok(Token::Le),
                ('>', false) => Ok(Token::Gt),
                ('>', true) => Ok(Token::Ge),
                _ => Err(self.error(StfErrorKind::InvalidCharacter(character), left)),
            };
        }

        if character == '"' {
            self.bump();
            let start = self.index;
            self.take_while(|next| next != '"' && next != '\n');
            if self.peek() != Some('"') {
                return Err(self.error(StfErrorKind::UnterminatedQuotedIdentifier, left));
            }
            let value = self.source[start..self.index].to_owned();
            self.bump();
            return Ok(Token::Id(value));
        }

        if character.is_ascii_digit() {
            let value = self.take_while(|next| {
                next.is_ascii_hexdigit() || matches!(next, 'x' | 'X' | 'b' | 'B' | '*')
            });
            let digits = value.get(2..).unwrap_or_default();
            let valid = if value.starts_with("0x") || value.starts_with("0X") {
                !digits.is_empty()
                    && digits
                        .chars()
                        .all(|digit| digit.is_ascii_hexdigit() || digit == '*')
            } else if value.starts_with("0b") || value.starts_with("0B") {
                !digits.is_empty() && digits.chars().all(|digit| matches!(digit, '0' | '1' | '*'))
            } else {
                value.chars().all(|digit| digit.is_ascii_digit())
            };
            if !valid {
                return Err(self.error(StfErrorKind::InvalidNumber(value.to_owned()), left));
            }
            return Ok(if value.starts_with("0x") || value.starts_with("0X") {
                if value.contains('*') {
                    Token::TernaryHex(value.to_owned())
                } else {
                    Token::IntHex(value.to_owned())
                }
            } else if value.starts_with("0b") || value.starts_with("0B") {
                Token::IntBinary(value.to_owned())
            } else {
                Token::IntDecimal(value.to_owned())
            });
        }

        if character == '$' || character == '_' || character.is_ascii_alphabetic() {
            let identifier = self.identifier();
            let token = match identifier.as_str() {
                "add" => Token::Add,
                "all" => Token::All,
                "bytes" => Token::Bytes,
                "check_counter" => Token::CheckCounter,
                "expect" => {
                    self.mode = Mode::PacketData;
                    Token::Expect
                }
                "mirroring_add" => Token::MirroringAdd,
                "mirroring_add_mc" => Token::MirroringAddMc,
                "mirroring_get" => Token::MirroringGet,
                "no_packet" => Token::NoPacket,
                "packet" => {
                    self.mode = Mode::PacketData;
                    Token::Packet
                }
                "packets" => Token::Packets,
                "remove" => Token::Remove,
                "setdefault" => Token::SetDefault,
                "mc_mgrp_create" => Token::McGroupCreate,
                "mc_node_create" => Token::McNodeCreate,
                "mc_node_associate" => Token::McNodeAssociate,
                "register_read" => Token::RegisterRead,
                "register_write" => Token::RegisterWrite,
                "register_reset" => Token::RegisterReset,
                "wait" => Token::Wait,
                _ => Token::Id(identifier),
            };
            return Ok(token);
        }

        self.bump();
        Err(self.error(StfErrorKind::InvalidCharacter(character), left))
    }

    fn packet_token(&mut self) -> Result<Token, StfError> {
        let left = self.source_position();
        let Some(character) = self.peek() else {
            self.mode = Mode::Keyword;
            self.finished = true;
            return Ok(Token::End);
        };
        if character == '$' {
            self.bump();
            self.mode = Mode::Keyword;
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
            let data = self.take_while(|next| next.is_ascii_hexdigit()).to_owned();
            return Ok(if data.chars().all(|next| next.is_ascii_digit()) {
                Token::DataDecimal(data)
            } else {
                Token::DataHex(data)
            });
        }
        self.bump();
        Err(self.error(StfErrorKind::InvalidCharacter(character), left))
    }
}

impl Iterator for Lexer<'_> {
    type Item = Result<(usize, Token, usize), StfError>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.finished {
            return None;
        }
        loop {
            match self.mode {
                Mode::Keyword => self.skip_keyword_layout(),
                Mode::PacketData => {
                    self.take_while(|character| matches!(character, ' ' | '\t' | '\r'));
                    if self.peek() == Some('\n') {
                        self.take_while(|character| character == '\n');
                        self.mode = Mode::Keyword;
                        continue;
                    }
                }
            }
            break;
        }
        let left = self.index;
        let token = match self.mode {
            Mode::Keyword => self.keyword_token(),
            Mode::PacketData => self.packet_token(),
        };
        let right = self.index;
        Some(token.map(|token| (left, token, right)))
    }
}
