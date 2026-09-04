//! Lazy context-sensitive tokenization for preprocessed P4
//!
//! `Iterator::next` asks `Lexer::lex` for one token. `Lexer::tokenize` first
//! skips ordinary whitespace and comments, constructs literal values, and
//! recognizes fixed tokens by maximal munch. `lex` emits a name first and its
//! context-sensitive identifier/type-name classification on the following
//! iteration, then distinguishes template angles.
//!
//! The two-part name token is a parser synchronization point: an LR parser may
//! request one token of lookahead before reducing the preceding declaration.
//! Emitting the spelling first lets that reduction update the name-resolution
//! context before the lexer classifies the same spelling.
//!
//! The grammar needs a few one-token lookahead distinctions to remain
//! deterministic. `Lexer::classify_name` distinguishes type names that start
//! expressions, while `Lexer::disambiguate_token` marks block-leading colons,
//! trailing commas, and `error` member accesses without changing their source
//! spelling. The expression grammar also needs one `ShiftRight` token, so
//! `Lexer::disambiguate_angles` splits that token into closing angles only
//! while scanning nested type arguments.
//!
//! # Examples
//!
//! ```text
//! source: Header<bit<8>> value
//! lexer:  Name("Header"), TypeName, LeftAngleArgs, Bit, LeftAngleArgs,
//!         NumberInt(8), RightAngle, RightAngleShift, Name("value"), Identifier
//!
//! source: x >> 1
//! lexer:  Name("x"), Identifier, ShiftRight, NumberInt(1)
//! ```

use std::{collections::VecDeque, rc::Rc};

use num_bigint::BigInt;

use crate::{
    lang::{
        common::{
            notation::{atom::Atom, mixfix::Mixfix},
            source::{Phrase, Position, Span},
        },
        data::{
            typ,
            value::{Value, make},
        },
        xl::num::Natural,
    },
    phrase,
};

use super::{
    context::{Context, IdentKind},
    error::{LexErrorKind, P4Error},
};

// == Tokens

/// A token consumed by the P4 grammar
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Token {
    End,
    TypeName,
    /// Type name at the start of a postfix expression
    TypeNameExpression,
    Identifier,
    Name(Rc<Value>),
    StringLiteral(Rc<Value>),
    NumberInt(Rc<Value>, String),
    Number(Rc<Value>, String),
    LessEqual,
    GreaterEqual,
    ShiftLeft,
    And,
    Or,
    NotEqual,
    Equal,
    /// Parser-only spelling of `>>` in an expression
    ShiftRight,
    Plus,
    Minus,
    PlusSaturating,
    MinusSaturating,
    Multiply,
    Invalid,
    Divide,
    Modulo,
    BitOr,
    BitAnd,
    BitXor,
    Complement,
    LeftBracket,
    RightBracket,
    LeftBrace,
    RightBrace,
    LeftAngle,
    LeftAngleArgs,
    RightAngle,
    RightAngleShift,
    LeftParen,
    RightParen,
    Assign,
    Colon,
    /// Parser-only colon immediately before a block or annotation
    BlockColon,
    Comma,
    /// Parser-only comma immediately before `}` or `]`
    TrailingComma,
    Question,
    Dot,
    Not,
    Semicolon,
    At,
    PlusPlus,
    PlusColon,
    DontCare,
    Mask,
    Dots,
    Range,
    True,
    False,
    Abstract,
    Action,
    Actions,
    Apply,
    Bool,
    Bit,
    Break,
    Const,
    Continue,
    Control,
    Default,
    Else,
    Entries,
    Enum,
    Error,
    /// Parser-only `error` immediately before member access
    ErrorExpression,
    Exit,
    Extern,
    Header,
    HeaderUnion,
    If,
    In,
    InOut,
    For,
    Int,
    Key,
    List,
    Select,
    MatchKind,
    Out,
    Package,
    Parser,
    Priority,
    Return,
    State,
    String,
    Struct,
    Switch,
    Table,
    This,
    Transition,
    Tuple,
    Typedef,
    Type,
    ValueSet,
    Varbit,
    Void,
    Pragma,
    PragmaEnd,
    PlusAssign,
    PlusSaturatingAssign,
    MinusAssign,
    MinusSaturatingAssign,
    MultiplyAssign,
    DivideAssign,
    ModuloAssign,
    ShiftLeftAssign,
    ShiftRightAssign,
    BitAndAssign,
    BitXorAssign,
    BitOrAssign,
    UnexpectedToken(Rc<Value>),
}

#[derive(Clone, Copy, Debug)]
enum LexerState {
    Regular,
    Pragma,
    Template,
}

// == Lexer

/// A lazy P4 token stream sharing the parser's name-resolution context
pub struct Lexer<'source> {
    source: &'source str,
    index: usize,
    file: Rc<str>,
    line: i64,
    column: i64,
    context: Rc<Context>,
    state: LexerState,
    pending: VecDeque<Phrase<Token>>,
    deferred_classification: Option<(Rc<Value>, Span, LexerState)>,
    template_depth: usize,
    finished: bool,
}

// - Construction

impl<'source> Lexer<'source> {
    /// Tokenizes preprocessed `source` using context-sensitive name classes
    pub fn new(file: Rc<str>, source: &'source str, context: Rc<Context>) -> Self {
        Self {
            source,
            index: 0,
            file,
            line: 1,
            column: 0,
            context,
            state: LexerState::Regular,
            pending: VecDeque::new(),
            deferred_classification: None,
            template_depth: 0,
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

    fn error(&self, kind: LexErrorKind, pos_l: Position) -> P4Error {
        P4Error::new(kind, self.span_from(pos_l))
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

    fn take_while(&mut self, mut predicate: impl FnMut(char) -> bool) -> &'source str {
        let start = self.index;
        while let Some(character) = self.source[self.index..].chars().next() {
            if !predicate(character) {
                break;
            }
            self.bump();
        }
        &self.source[start..self.index]
    }

    // - Token emission

    fn lex(&mut self) -> Option<Result<Phrase<Token>, P4Error>> {
        if let Some((value, span, next)) = self.deferred_classification.take() {
            return Some(Ok(self.classify_name(&value, &span, next)));
        }
        if let Some(token) = self.pending.pop_front() {
            return Some(Ok(token));
        }
        loop {
            let token = match self.tokenize()? {
                Ok(token) => token,
                Err(error) => return Some(Err(error)),
            };
            let span = token.span.clone();
            let token = self.disambiguate_token(token.node);
            let token = match self.state {
                LexerState::Regular => match token {
                    Token::Name(value) => {
                        self.defer_classification(&value, &span, LexerState::Regular);
                        Token::Name(value)
                    }
                    Token::Pragma => {
                        self.state = LexerState::Pragma;
                        Token::Pragma
                    }
                    token @ (Token::Bit
                    | Token::Int
                    | Token::Varbit
                    | Token::List
                    | Token::Tuple
                    | Token::ValueSet) => {
                        self.state = LexerState::Template;
                        token
                    }
                    Token::PragmaEnd => continue,
                    token => token,
                },
                LexerState::Pragma => match token {
                    Token::PragmaEnd => {
                        self.state = LexerState::Regular;
                        Token::PragmaEnd
                    }
                    Token::Name(value) => {
                        self.defer_classification(&value, &span, LexerState::Pragma);
                        Token::Name(value)
                    }
                    token => token,
                },
                LexerState::Template => match token {
                    Token::LeftAngle | Token::LeftAngleArgs => {
                        self.state = LexerState::Regular;
                        Token::LeftAngleArgs
                    }
                    Token::Name(value) => {
                        self.defer_classification(&value, &span, LexerState::Regular);
                        Token::Name(value)
                    }
                    Token::Pragma => {
                        self.state = LexerState::Pragma;
                        Token::Pragma
                    }
                    Token::PragmaEnd => continue,
                    token => {
                        self.state = LexerState::Regular;
                        token
                    }
                },
            };
            let (token, span) = self.disambiguate_angles(token, span);
            return Some(Ok(phrase!(node: token, span: span)));
        }
    }

    // - Template angle disambiguation

    fn disambiguate_angles(&mut self, token: Token, span: Span) -> (Token, Span) {
        match token {
            Token::LeftAngleArgs => {
                self.template_depth += 1;
                (Token::LeftAngleArgs, span)
            }
            Token::RightAngle if self.template_depth > 0 => {
                self.template_depth -= 1;
                (Token::RightAngle, span)
            }
            Token::ShiftRight if self.template_depth > 0 => {
                let pos_middle = Position::new(
                    Rc::clone(&span.left.file),
                    span.left.line,
                    span.left.column + 1,
                );
                let second = if self.template_depth > 1 {
                    self.template_depth -= 2;
                    Token::RightAngleShift
                } else {
                    self.template_depth = 0;
                    Token::RightAngle
                };
                let span_second = Span::new(pos_middle.clone(), span.right.clone());
                let token_second = phrase!(node: second, span: span_second);
                self.pending.push_back(token_second);
                let span_first = Span::new(span.left, pos_middle);
                (Token::RightAngle, span_first)
            }
            token => (token, span),
        }
    }

    // - Contextual token disambiguation

    fn disambiguate_token(&mut self, mut token: Token) -> Token {
        if self.context.take_template_expected() && token == Token::LeftAngle {
            token = Token::LeftAngleArgs;
        }
        if token == Token::Comma && self.remaining_significant().starts_with(['}', ']']) {
            token = Token::TrailingComma;
        } else if token == Token::Error && self.remaining_significant().starts_with('.') {
            token = Token::ErrorExpression;
        }
        if token == Token::Colon
            && (self.remaining_significant().starts_with(['{', '@'])
                || self.remaining_significant().starts_with("#pragma"))
        {
            token = Token::BlockColon;
        }
        token
    }

    // - Name classification

    fn defer_classification(&mut self, value: &Rc<Value>, span: &Span, next: LexerState) {
        self.deferred_classification = Some((Rc::clone(value), span.clone(), next));
    }

    fn classify_name(&mut self, value: &Rc<Value>, span: &Span, next: LexerState) -> Phrase<Token> {
        let name = match &value.node {
            crate::lang::data::value::ValueKind::Text(name) => name,
            _ => return phrase!(node: Token::Identifier, span: span.clone()),
        };
        let (token, template_expected) = match self.context.get_kind(name) {
            IdentKind::TypeName { has_params, .. } => {
                let token = if self.type_name_starts_expression() {
                    Token::TypeNameExpression
                } else {
                    Token::TypeName
                };
                let template_expected = has_params
                    || self.remaining_significant().starts_with('<')
                    || self.template_arguments_follow();
                (token, template_expected)
            }
            IdentKind::Ident { has_params, .. } => (
                Token::Identifier,
                has_params || self.adjacent_template_arguments_follow(),
            ),
        };
        self.state = if template_expected {
            LexerState::Template
        } else {
            next
        };
        phrase!(node: token, span: span.clone())
    }

    fn type_name_starts_expression(&self) -> bool {
        let rest = self.remaining_significant();
        rest.starts_with(['.', '(']) || self.template_arguments_follow()
    }

    fn template_arguments_follow(&self) -> bool {
        angle_suffix(self.remaining_significant())
            .is_some_and(|suffix| suffix.trim_start().starts_with('('))
    }

    fn adjacent_template_arguments_follow(&self) -> bool {
        angle_suffix(&self.source[self.index..])
            .is_some_and(|suffix| suffix.trim_start().starts_with('('))
    }

    fn remaining_significant(&self) -> &str {
        let mut rest = &self.source[self.index..];
        loop {
            rest = rest.trim_start();
            if let Some(after) = rest.strip_prefix("//") {
                rest = after.find('\n').map_or("", |index| &after[index + 1..]);
            } else if let Some(after) = rest.strip_prefix("/*") {
                rest = after.find("*/").map_or("", |index| &after[index + 2..]);
            } else {
                return rest;
            }
        }
    }

    // - Raw scanning

    fn tokenize(&mut self) -> Option<Result<Phrase<Token>, P4Error>> {
        loop {
            if self.index == self.source.len() {
                if self.finished {
                    return None;
                }
                self.finished = true;
                let position = self.source_position();
                let span = Span::new(position.clone(), position);
                return Some(Ok(phrase!(node: Token::End, span: span)));
            }
            let pos_l = self.source_position();
            let rest = &self.source[self.index..];

            if rest.starts_with("/*") {
                self.bump();
                self.bump();
                let mut crossed_newline = false;
                while self.index < self.source.len() && !self.source[self.index..].starts_with("*/")
                {
                    crossed_newline |= self.bump() == Some('\n');
                }
                if self.index == self.source.len() {
                    return Some(Err(self.error(LexErrorKind::UnterminatedComment, pos_l)));
                }
                self.bump();
                self.bump();
                if crossed_newline {
                    let span = self.span_from(pos_l);
                    return Some(Ok(phrase!(node: Token::PragmaEnd, span: span)));
                }
                continue;
            }
            if rest.starts_with("//") {
                self.take_while(|character| character != '\n');
                continue;
            }
            if rest.starts_with('\n') {
                self.bump();
                let span = self.span_from(pos_l);
                return Some(Ok(phrase!(node: Token::PragmaEnd, span: span)));
            }
            if rest.starts_with('"') {
                return Some(self.string_token(pos_l));
            }
            if rest.starts_with([' ', '\t', '\u{000c}', '\r']) {
                self.take_while(|character| matches!(character, ' ' | '\t' | '\u{000c}' | '\r'));
                continue;
            }
            if rest.starts_with('#') {
                self.preprocessor_line();
                continue;
            }
            if rest.starts_with("@pragma") {
                for _ in 0.."@pragma".len() {
                    self.bump();
                }
                let span = self.span_from(pos_l);
                return Some(Ok(phrase!(node: Token::Pragma, span: span)));
            }
            if rest.as_bytes()[0].is_ascii_digit() {
                return Some(self.number_token(pos_l));
            }
            if is_name_start(rest.as_bytes()[0]) {
                let start = self.index;
                self.bump();
                self.take_while(|character| character.is_ascii_alphanumeric() || character == '_');
                let text = &self.source[start..self.index];
                let token = keyword(text).unwrap_or_else(|| {
                    let value = make::text(text.to_owned(), self.span_from(pos_l.clone()));
                    Token::Name(value)
                });
                let span = self.span_from(pos_l);
                return Some(Ok(phrase!(node: token, span: span)));
            }

            if let Some((spelling, token)) = fixed_token(rest) {
                for _ in 0..spelling.len() {
                    self.bump();
                }
                let span = self.span_from(pos_l);
                return Some(Ok(phrase!(node: token, span: span)));
            }

            let text = self.bump().expect("source is not empty").to_string();
            let span = self.span_from(pos_l);
            let value = make::text(text, span.clone());
            let token = Token::UnexpectedToken(value);
            return Some(Ok(phrase!(node: token, span: span)));
        }
    }

    // - String literals

    fn string_token(&mut self, pos_l: Position) -> Result<Phrase<Token>, P4Error> {
        self.bump();
        let mut text = String::new();
        loop {
            let Some(character) = self.bump() else {
                return Err(self.error(LexErrorKind::UnterminatedString, pos_l));
            };
            match character {
                '"' => break,
                '\\' => {
                    let Some(escaped) = self.bump() else {
                        return Err(self.error(LexErrorKind::UnterminatedString, pos_l));
                    };
                    match escaped {
                        '"' => text.push('"'),
                        'n' => text.push('\n'),
                        '\\' => text.push('\\'),
                        escaped => {
                            return Err(self.error(
                                LexErrorKind::UnsupportedEscape(format!("\\{escaped}")),
                                pos_l,
                            ));
                        }
                    }
                }
                character => text.push(character),
            }
        }
        let span = self.span_from(pos_l);
        let value = make::text(text, span.clone());
        let token = Token::StringLiteral(value);
        Ok(phrase!(node: token, span: span))
    }

    // - Integer literals

    fn number_token(&mut self, pos_l: Position) -> Result<Phrase<Token>, P4Error> {
        let start = self.index;
        let rest = &self.source[start..];
        let int_len = integer_lexeme_len(rest);
        let sized = sized_integer_lexeme(rest);
        let (lexeme_len, sign_index) = match sized {
            Some((sized_len, sign_index)) if sized_len > int_len => (sized_len, Some(sign_index)),
            _ => (int_len, None),
        };
        for _ in 0..lexeme_len {
            self.bump();
        }
        let spelling = &self.source[start..self.index];
        let (value, lexeme) = match sign_index {
            Some(index) => {
                let width = &spelling[..index];
                let sign = spelling.as_bytes()[index] as char;
                let digits = &spelling[index + 1..];
                let int = parse_integer(digits).ok_or_else(|| {
                    self.error(
                        LexErrorKind::InvalidInteger(spelling.to_owned()),
                        pos_l.clone(),
                    )
                })?;
                let int_width = parse_integer(width).ok_or_else(|| {
                    self.error(
                        LexErrorKind::InvalidInteger(spelling.to_owned()),
                        pos_l.clone(),
                    )
                })?;
                if sign == 's' && int_width < BigInt::from(2) {
                    return Err(self.error(LexErrorKind::SignedWidth, pos_l));
                }
                let span = self.span_from(pos_l.clone());
                let nat_width = Natural::try_from(int_width).map_err(|_| {
                    self.error(
                        LexErrorKind::InvalidInteger(spelling.to_owned()),
                        pos_l.clone(),
                    )
                })?;
                let value_width = make::nat(nat_width, span.clone());
                let value_int = make::int(int, span.clone());
                let atom = phrase!(
                    node: Atom::Keyword(sign.to_ascii_uppercase().to_string()),
                    span: span.clone()
                );
                let value_case = Mixfix::Seq(vec![
                    Mixfix::Arg(value_width),
                    Mixfix::Atom(atom),
                    Mixfix::Arg(value_int),
                ]);
                let id_typ = phrase!(node: "integerLiteral".to_owned(), span: Span::default());
                let value = make::case(&typ::make::var(id_typ, vec![]), value_case, span);
                (value, digits.to_owned())
            }
            _ => {
                let int = parse_integer(spelling).ok_or_else(|| {
                    self.error(
                        LexErrorKind::InvalidInteger(spelling.to_owned()),
                        pos_l.clone(),
                    )
                })?;
                let span = self.span_from(pos_l.clone());
                (make::int(int, span), spelling.to_owned())
            }
        };
        let span = self.span_from(pos_l);
        let token = if sign_index.is_some() {
            Token::Number(value, lexeme)
        } else {
            Token::NumberInt(value, lexeme)
        };
        Ok(phrase!(node: token, span: span))
    }

    // - Preprocessor line markers

    fn preprocessor_line(&mut self) {
        self.take_while(|character| character != '\n');
        let text_line = self.source[..self.index]
            .rsplit_once('\n')
            .map_or(&self.source[..self.index], |(_, line)| line);
        let text_directive = &text_line[1..];
        let text_before_path = text_directive
            .split_once('"')
            .map_or(text_directive, |(before, _)| before);
        let line = text_before_path
            .split(|character: char| !character.is_ascii_digit() && character != '_')
            .filter_map(|text| text.replace('_', "").parse::<i64>().ok())
            .next_back();
        if let Some(line) = line {
            self.line = line;
        }
        if let Some(quote_l) = text_directive.find('"') {
            let path = &text_directive[quote_l + 1..];
            if let Some(quote_r) = path.find('"') {
                self.file = Rc::from(&path[..quote_r]);
            }
        }
        if self.index < self.source.len() {
            self.index += 1;
            self.column = 0;
        }
    }
}

// == Token lookahead

fn angle_suffix(rest: &str) -> Option<&str> {
    if !rest.starts_with('<') {
        return None;
    }
    let mut depth = 0usize;
    for (index, character) in rest.char_indices() {
        match character {
            '<' => depth += 1,
            '>' => {
                depth = depth.checked_sub(1)?;
                if depth == 0 {
                    return Some(&rest[index + character.len_utf8()..]);
                }
            }
            _ => {}
        }
    }
    None
}

impl Iterator for Lexer<'_> {
    type Item = Result<Phrase<Token>, P4Error>;

    fn next(&mut self) -> Option<Self::Item> {
        self.lex()
    }
}

// == Integer literal helpers

fn integer_lexeme_len(rest: &str) -> usize {
    let bytes = rest.as_bytes();
    if !bytes.first().is_some_and(u8::is_ascii_digit) {
        return 0;
    }

    let int_len = bytes
        .iter()
        .take_while(|byte| byte.is_ascii_digit() || **byte == b'_')
        .count();
    let mut len = int_len;
    for (prefix, valid_digit) in [
        ("0x", is_hex_digit as fn(u8) -> bool),
        ("0X", is_hex_digit),
        ("0d", is_decimal_digit),
        ("0D", is_decimal_digit),
        ("0o", is_octal_digit),
        ("0O", is_octal_digit),
        ("0b", is_binary_digit),
        ("0B", is_binary_digit),
    ] {
        let Some(digits) = rest.strip_prefix(prefix) else {
            continue;
        };
        let digits_len = digits
            .bytes()
            .take_while(|byte| valid_digit(*byte) || *byte == b'_')
            .count();
        if digits_len > 0 {
            len = len.max(prefix.len() + digits_len);
        }
    }
    len
}

fn sized_integer_lexeme(rest: &str) -> Option<(usize, usize)> {
    let sign_index = rest.bytes().take_while(u8::is_ascii_digit).count();
    let sign = *rest.as_bytes().get(sign_index)?;
    if !matches!(sign, b'w' | b's') {
        return None;
    }
    let integer_len = integer_lexeme_len(&rest[sign_index + 1..]);
    (integer_len > 0).then_some((sign_index + 1 + integer_len, sign_index))
}

fn is_hex_digit(byte: u8) -> bool {
    byte.is_ascii_hexdigit()
}

fn is_decimal_digit(byte: u8) -> bool {
    byte.is_ascii_digit()
}

fn is_octal_digit(byte: u8) -> bool {
    matches!(byte, b'0'..=b'7')
}

fn is_binary_digit(byte: u8) -> bool {
    matches!(byte, b'0' | b'1')
}

fn parse_integer(spelling: &str) -> Option<BigInt> {
    let sanitized = spelling.replace('_', "");
    let (radix, digits) = if let Some(digits) = sanitized
        .strip_prefix("0x")
        .or_else(|| sanitized.strip_prefix("0X"))
    {
        (16, digits)
    } else if let Some(digits) = sanitized
        .strip_prefix("0o")
        .or_else(|| sanitized.strip_prefix("0O"))
    {
        (8, digits)
    } else if let Some(digits) = sanitized
        .strip_prefix("0b")
        .or_else(|| sanitized.strip_prefix("0B"))
    {
        (2, digits)
    } else if let Some(digits) = sanitized
        .strip_prefix("0d")
        .or_else(|| sanitized.strip_prefix("0D"))
    {
        (10, digits)
    } else {
        (10, sanitized.as_str())
    };
    BigInt::parse_bytes(digits.as_bytes(), radix)
}

// == Names and keywords

fn is_name_start(byte: u8) -> bool {
    byte.is_ascii_alphabetic() || byte == b'_'
}

fn keyword(text: &str) -> Option<Token> {
    Some(match text {
        "abstract" => Token::Abstract,
        "action" => Token::Action,
        "actions" => Token::Actions,
        "apply" => Token::Apply,
        "bool" => Token::Bool,
        "bit" => Token::Bit,
        "break" => Token::Break,
        "const" => Token::Const,
        "continue" => Token::Continue,
        "control" => Token::Control,
        "default" => Token::Default,
        "else" => Token::Else,
        "entries" => Token::Entries,
        "enum" => Token::Enum,
        "error" => Token::Error,
        "exit" => Token::Exit,
        "extern" => Token::Extern,
        "header" => Token::Header,
        "header_union" => Token::HeaderUnion,
        "true" => Token::True,
        "false" => Token::False,
        "for" => Token::For,
        "if" => Token::If,
        "in" => Token::In,
        "inout" => Token::InOut,
        "int" => Token::Int,
        "key" => Token::Key,
        "list" => Token::List,
        "match_kind" => Token::MatchKind,
        "out" => Token::Out,
        "parser" => Token::Parser,
        "package" => Token::Package,
        "pragma" => Token::Pragma,
        "priority" => Token::Priority,
        "return" => Token::Return,
        "select" => Token::Select,
        "state" => Token::State,
        "string" => Token::String,
        "struct" => Token::Struct,
        "switch" => Token::Switch,
        "table" => Token::Table,
        "this" => Token::This,
        "transition" => Token::Transition,
        "tuple" => Token::Tuple,
        "typedef" => Token::Typedef,
        "type" => Token::Type,
        "value_set" => Token::ValueSet,
        "varbit" => Token::Varbit,
        "void" => Token::Void,
        "_" => Token::DontCare,
        _ => return None,
    })
}

// == Fixed tokens

fn fixed_token(rest: &str) -> Option<(&'static str, Token)> {
    let mut matched: Option<(&'static str, Token)> = None;
    for (spelling, token) in fixed_tokens() {
        if rest.starts_with(spelling)
            && matched
                .as_ref()
                .is_none_or(|(previous, _)| spelling.len() > previous.len())
        {
            matched = Some((*spelling, token.clone()));
        }
    }
    matched
}

fn fixed_tokens() -> &'static [(&'static str, Token)] {
    &[
        ("<=", Token::LessEqual),
        (">=", Token::GreaterEqual),
        ("<<", Token::ShiftLeft),
        // LALRPOP's precedence grammar requires one expression token
        (">>", Token::ShiftRight),
        ("&&", Token::And),
        ("||", Token::Or),
        ("!=", Token::NotEqual),
        ("==", Token::Equal),
        ("+:", Token::PlusColon),
        ("+", Token::Plus),
        ("-", Token::Minus),
        ("|+|", Token::PlusSaturating),
        ("|-|", Token::MinusSaturating),
        ("*", Token::Multiply),
        ("{#}", Token::Invalid),
        ("/", Token::Divide),
        ("%", Token::Modulo),
        ("|", Token::BitOr),
        ("&", Token::BitAnd),
        ("^", Token::BitXor),
        ("~", Token::Complement),
        ("[", Token::LeftBracket),
        ("]", Token::RightBracket),
        ("{", Token::LeftBrace),
        ("}", Token::RightBrace),
        ("<", Token::LeftAngle),
        (">", Token::RightAngle),
        ("(", Token::LeftParen),
        (")", Token::RightParen),
        ("!", Token::Not),
        (":", Token::Colon),
        (",", Token::Comma),
        ("?", Token::Question),
        (".", Token::Dot),
        ("=", Token::Assign),
        (";", Token::Semicolon),
        ("@", Token::At),
        ("++", Token::PlusPlus),
        ("&&&", Token::Mask),
        ("...", Token::Dots),
        ("..", Token::Range),
        ("+=", Token::PlusAssign),
        ("|+|=", Token::PlusSaturatingAssign),
        ("-=", Token::MinusAssign),
        ("|-|=", Token::MinusSaturatingAssign),
        ("*=", Token::MultiplyAssign),
        ("/=", Token::DivideAssign),
        ("%=", Token::ModuloAssign),
        ("<<=", Token::ShiftLeftAssign),
        (">>=", Token::ShiftRightAssign),
        ("&=", Token::BitAndAssign),
        ("^=", Token::BitXorAssign),
        ("|=", Token::BitOrAssign),
    ]
}
