//! Context-sensitive lexer for preprocessed P4 source.
//!
//! Raw spelling is scanned first, then contextual name classification and
//! angle-bracket disambiguation enqueue the parser tokens. For example, `>>`
//! inside two nested type arguments is emitted as two closing-angle tokens,
//! while the same spelling in an expression remains a shift.

use std::{collections::VecDeque, rc::Rc};

use num_bigint::BigInt;

use crate::{
    lang::{
        common::{
            notation::{atom::Atom, mixfix::Mixfix},
            source::{Phrase, Position, Span},
        },
        xl::num::Natural,
    },
    phrase,
    runtime::{
        types::typ,
        value::{ValueRef, make},
    },
};

use super::{
    context::{Context, IdentKind},
    error::{LexErrorKind, P4Error},
};

// == Tokens

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Token {
    End,
    TypeName,
    TypeNameExpression,
    Identifier,
    Name(ValueRef),
    StringLiteral(ValueRef),
    NumberInt(ValueRef, String),
    Number(ValueRef, String),
    UnexpectedToken(ValueRef),
    LessEqual,
    GreaterEqual,
    ShiftLeft,
    And,
    Or,
    NotEqual,
    Equal,
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
    BlockColon,
    Comma,
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
}

#[derive(Clone, Copy, Debug)]
enum State {
    Regular,
    Template,
    Pragma,
}

// == Lexer state

pub struct Lexer<'source> {
    source: &'source str,
    index: usize,
    file: Rc<str>,
    line: i64,
    column: i64,
    context: Rc<Context>,
    state: State,
    pending: VecDeque<Phrase<Token>>,
    deferred_classification: Option<(ValueRef, Span, State)>,
    template_depth: usize,
    finished: bool,
}

impl<'source> Lexer<'source> {
    pub fn new(file: Rc<str>, source: &'source str, context: Rc<Context>) -> Self {
        Self {
            source,
            index: 0,
            file,
            line: 1,
            column: 0,
            context,
            state: State::Regular,
            pending: VecDeque::new(),
            deferred_classification: None,
            template_depth: 0,
            finished: false,
        }
    }

    fn source_position(&self) -> Position {
        Position::new(Rc::clone(&self.file), self.line, self.column)
    }

    fn span_from(&self, left: Position) -> Span {
        Span::new(left, self.source_position())
    }

    fn error(&self, kind: LexErrorKind, left: Position) -> P4Error {
        P4Error::new(kind, self.span_from(left))
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

    fn next_token(&mut self) -> Option<Result<Phrase<Token>, P4Error>> {
        if let Some((value, span, next)) = self.deferred_classification.take() {
            return Some(Ok(self.classify_name(&value, &span, next)));
        }
        if let Some(token) = self.pending.pop_front() {
            return Some(Ok(token));
        }
        loop {
            let token = match self.raw_token()? {
                Ok(token) => token,
                Err(error) => return Some(Err(error)),
            };
            let span = token.span.clone();
            let token = if self.context.take_template_expected() && token.node == Token::LeftAngle {
                phrase!(node: Token::LeftAngleArgs, span: span.clone())
            } else {
                token
            };
            let token = if token.node == Token::Comma
                && self.remaining_significant().starts_with(['}', ']'])
            {
                phrase!(node: Token::TrailingComma, span: span.clone())
            } else if token.node == Token::Error && self.remaining_significant().starts_with('.') {
                phrase!(node: Token::ErrorExpression, span: span.clone())
            } else {
                token
            };
            let token = if token.node == Token::Colon
                && (self.remaining_significant().starts_with(['{', '@'])
                    || self.remaining_significant().starts_with("#pragma"))
            {
                phrase!(node: Token::BlockColon, span: span.clone())
            } else {
                token
            };
            let token = match self.state {
                State::Regular => match token.node {
                    Token::Name(value) => {
                        self.defer_classification(&value, &span, State::Regular);
                        Token::Name(value)
                    }
                    Token::Pragma => {
                        self.state = State::Pragma;
                        Token::Pragma
                    }
                    token @ (Token::Bit
                    | Token::Int
                    | Token::Varbit
                    | Token::List
                    | Token::Tuple
                    | Token::ValueSet) => {
                        self.state = State::Template;
                        token
                    }
                    Token::PragmaEnd => continue,
                    token => token,
                },
                State::Template => match token.node {
                    Token::LeftAngle => {
                        self.state = State::Regular;
                        Token::LeftAngleArgs
                    }
                    Token::Name(value) => {
                        self.defer_classification(&value, &span, State::Regular);
                        Token::Name(value)
                    }
                    Token::Pragma => {
                        self.state = State::Pragma;
                        Token::Pragma
                    }
                    Token::PragmaEnd => continue,
                    token => {
                        self.state = State::Regular;
                        token
                    }
                },
                State::Pragma => match token.node {
                    Token::PragmaEnd => {
                        self.state = State::Regular;
                        Token::PragmaEnd
                    }
                    Token::Name(value) => {
                        self.defer_classification(&value, &span, State::Pragma);
                        Token::Name(value)
                    }
                    token => token,
                },
            };
            let (token, span) = self.disambiguate_angles(token, span);
            return Some(Ok(phrase!(node: token, span: span)));
        }
    }

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
                let middle = Position::new(
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
                self.pending.push_back(phrase!(
                    node: second,
                    span: Span::new(middle.clone(), span.right.clone())
                ));
                (Token::RightAngle, Span::new(span.left, middle))
            }
            token => (token, span),
        }
    }

    fn defer_classification(&mut self, value: &ValueRef, span: &Span, next: State) {
        self.deferred_classification = Some((Rc::clone(value), span.clone(), next));
    }

    fn classify_name(&mut self, value: &ValueRef, span: &Span, next: State) -> Phrase<Token> {
        let name = match &value.node {
            crate::runtime::value::ValueKind::Text(name) => name,
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
            State::Template
        } else {
            next
        };
        phrase!(node: token, span: span.clone())
    }

    fn type_name_starts_expression(&self) -> bool {
        let rest = self.remaining_significant();
        if rest.starts_with(['.', '(']) {
            return true;
        }
        self.template_arguments_follow()
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

    fn raw_token(&mut self) -> Option<Result<Phrase<Token>, P4Error>> {
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
            let left = self.source_position();
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
                    return Some(Err(self.error(LexErrorKind::UnterminatedComment, left)));
                }
                self.bump();
                self.bump();
                if crossed_newline {
                    let span = self.span_from(left);
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
                let span = self.span_from(left);
                return Some(Ok(phrase!(node: Token::PragmaEnd, span: span)));
            }
            if rest.starts_with([' ', '\t', '\u{000c}', '\r']) {
                self.take_while(|character| matches!(character, ' ' | '\t' | '\u{000c}' | '\r'));
                continue;
            }
            if rest.starts_with('#') {
                self.preprocessor_line();
                continue;
            }
            if rest.starts_with('"') {
                return Some(self.string_token(left));
            }
            if rest.as_bytes()[0].is_ascii_digit() {
                return Some(self.number_token(left));
            }
            if is_name_start(rest.as_bytes()[0]) {
                let start = self.index;
                self.bump();
                self.take_while(|character| character.is_ascii_alphanumeric() || character == '_');
                let text = &self.source[start..self.index];
                let token = keyword(text).unwrap_or_else(|| {
                    let value = make::text(text.to_owned(), self.span_from(left.clone()));
                    Token::Name(value)
                });
                let span = self.span_from(left);
                return Some(Ok(phrase!(node: token, span: span)));
            }

            if rest.starts_with("@pragma") {
                for _ in 0.."@pragma".len() {
                    self.bump();
                }
                let span = self.span_from(left);
                return Some(Ok(phrase!(node: Token::Pragma, span: span)));
            }

            for (spelling, token) in operators() {
                if rest.starts_with(spelling) {
                    for _ in 0..spelling.len() {
                        self.bump();
                    }
                    let span = self.span_from(left);
                    return Some(Ok(phrase!(node: token.clone(), span: span)));
                }
            }

            let text = self.bump().expect("source is not empty").to_string();
            let span = self.span_from(left);
            let value = make::text(text, span.clone());
            return Some(Ok(phrase!(node: Token::UnexpectedToken(value), span: span)));
        }
    }

    fn string_token(&mut self, left: Position) -> Result<Phrase<Token>, P4Error> {
        self.bump();
        let mut text = String::new();
        loop {
            let Some(character) = self.bump() else {
                return Err(self.error(LexErrorKind::UnterminatedString, left));
            };
            match character {
                '"' => break,
                '\\' => {
                    let Some(escaped) = self.bump() else {
                        return Err(self.error(LexErrorKind::UnterminatedString, left));
                    };
                    match escaped {
                        '"' => text.push('"'),
                        'n' => text.push('\n'),
                        '\\' => text.push('\\'),
                        escaped => {
                            return Err(self.error(
                                LexErrorKind::UnsupportedEscape(format!("\\{escaped}")),
                                left,
                            ));
                        }
                    }
                }
                character => text.push(character),
            }
        }
        let span = self.span_from(left);
        let value = make::text(text, span.clone());
        Ok(phrase!(node: Token::StringLiteral(value), span: span))
    }

    fn number_token(&mut self, left: Position) -> Result<Phrase<Token>, P4Error> {
        let start = self.index;
        self.take_while(|character| character.is_ascii_alphanumeric() || character == '_');
        let spelling = &self.source[start..self.index];
        let width_end = spelling.find(['w', 's']);
        let (token, lexeme) = match width_end {
            Some(index) if index > 0 => {
                let width = &spelling[..index];
                let sign = spelling.as_bytes()[index] as char;
                let digits = &spelling[index + 1..];
                let integer = parse_integer(digits).ok_or_else(|| {
                    self.error(
                        LexErrorKind::InvalidInteger(spelling.to_owned()),
                        left.clone(),
                    )
                })?;
                let width_integer = parse_integer(width).ok_or_else(|| {
                    self.error(
                        LexErrorKind::InvalidInteger(spelling.to_owned()),
                        left.clone(),
                    )
                })?;
                if sign == 's' && width_integer < BigInt::from(2) {
                    return Err(self.error(LexErrorKind::SignedWidth, left));
                }
                let span = self.span_from(left.clone());
                let width = Natural::try_from(width_integer).map_err(|_| {
                    self.error(
                        LexErrorKind::InvalidInteger(spelling.to_owned()),
                        left.clone(),
                    )
                })?;
                let value_width = make::nat(width, span.clone());
                let value_integer = make::int(integer, span.clone());
                let atom = phrase!(
                    node: Atom::Keyword(sign.to_ascii_uppercase().to_string()),
                    span: span.clone()
                );
                let value_case = Mixfix::Seq(vec![
                    Mixfix::Arg(value_width),
                    Mixfix::Atom(atom),
                    Mixfix::Arg(value_integer),
                ]);
                let type_id = phrase!(node: "integerLiteral".to_owned(), span: Span::default());
                let value = make::case(&typ::var(type_id, vec![]), value_case, span);
                (value, digits.to_owned())
            }
            _ => {
                let integer = parse_integer(spelling).ok_or_else(|| {
                    self.error(
                        LexErrorKind::InvalidInteger(spelling.to_owned()),
                        left.clone(),
                    )
                })?;
                let span = self.span_from(left.clone());
                (make::int(integer, span), spelling.to_owned())
            }
        };
        let span = self.span_from(left);
        let token = if width_end.is_some() {
            Token::Number(token, lexeme)
        } else {
            Token::NumberInt(token, lexeme)
        };
        Ok(phrase!(node: token, span: span))
    }

    fn preprocessor_line(&mut self) {
        self.take_while(|character| character != '\n');
        let line_text = self.source[..self.index]
            .rsplit_once('\n')
            .map_or(&self.source[..self.index], |(_, line)| line);
        let mut fields = line_text[1..].split_whitespace();
        if let Some(line) = fields.next().and_then(|line| line.parse::<i64>().ok()) {
            self.line = line;
        }
        if let Some(path) = fields
            .next()
            .and_then(|path| path.strip_prefix('"'))
            .and_then(|path| path.strip_suffix('"'))
        {
            self.file = Rc::from(path);
        }
        if self.index < self.source.len() {
            self.index += 1;
            self.column = 0;
        }
    }
}

// == Angle lookahead and token stream

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
        self.next_token()
    }
}

// == Literal and fixed-token classification

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
        "@pragma" => Token::Pragma,
        _ => return None,
    })
}

// - Operators

fn operators() -> &'static [(&'static str, Token)] {
    &[
        ("|+|=", Token::PlusSaturatingAssign),
        ("|-|=", Token::MinusSaturatingAssign),
        ("{#}", Token::Invalid),
        ("&&&", Token::Mask),
        ("...", Token::Dots),
        (">>=", Token::ShiftRightAssign),
        ("<<=", Token::ShiftLeftAssign),
        ("|+|", Token::PlusSaturating),
        ("|-|", Token::MinusSaturating),
        ("<=", Token::LessEqual),
        (">=", Token::GreaterEqual),
        (">>", Token::ShiftRight),
        ("<<", Token::ShiftLeft),
        ("&&", Token::And),
        ("||", Token::Or),
        ("!=", Token::NotEqual),
        ("==", Token::Equal),
        ("+:", Token::PlusColon),
        ("++", Token::PlusPlus),
        ("..", Token::Range),
        ("+=", Token::PlusAssign),
        ("-=", Token::MinusAssign),
        ("*=", Token::MultiplyAssign),
        ("/=", Token::DivideAssign),
        ("%=", Token::ModuloAssign),
        ("&=", Token::BitAndAssign),
        ("^=", Token::BitXorAssign),
        ("|=", Token::BitOrAssign),
        ("+", Token::Plus),
        ("-", Token::Minus),
        ("*", Token::Multiply),
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
    ]
}
