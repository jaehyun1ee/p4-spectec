use std::{cell::RefCell, collections::BTreeSet, iter::Peekable};

use crate::lang::{
    common::{
        notation::atom::Atom,
        source::{Position, Span, Spanned},
    },
    el::ast::{DefTypKind, Exp, ExpKind, Hint, Iter, NotTypKind, Path, PathKind, Typ},
};

use super::{
    error::{FrontendError, LexError, SyntaxError, SyntaxErrorKind},
    lexer::Token,
};

/// Variable bindings shared by parser actions and contextual lexing
#[derive(Default)]
pub(crate) struct ParserContext {
    variables: RefCell<BTreeSet<String>>,
    scopes: RefCell<Vec<BTreeSet<String>>>,
    positions: RefCell<Vec<Position>>,
}

impl ParserContext {
    pub(crate) fn is_var(&self, identifier: &str) -> bool {
        self.variables.borrow().contains(identifier)
    }

    pub(crate) fn bind(&self, identifier: &str) {
        self.variables.borrow_mut().insert(identifier.to_owned());
    }

    pub(crate) fn enter_scope(&self) {
        self.scopes
            .borrow_mut()
            .push(self.variables.borrow().clone());
    }

    pub(crate) fn exit_scope(&self) {
        let variables = self
            .scopes
            .borrow_mut()
            .pop()
            .expect("parser scope actions are balanced");
        *self.variables.borrow_mut() = variables;
    }

    pub(crate) fn intern_position(&self, position: Position) -> ParserLocation {
        let mut positions = self.positions.borrow_mut();
        let location = ParserLocation(positions.len());
        positions.push(position);
        location
    }

    pub(crate) fn position(&self, location: ParserLocation) -> Position {
        self.positions.borrow()[location.0].clone()
    }

    pub(crate) fn span(&self, left: ParserLocation, right: ParserLocation) -> Span {
        Span::new(self.position(left), self.position(right))
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) struct ParserLocation(usize);

pub(crate) fn spanned<T>(
    context: &ParserContext,
    node: T,
    left: ParserLocation,
    right: ParserLocation,
) -> Spanned<T> {
    Spanned::new(node, context.span(left, right))
}

pub(crate) fn span_between<T, U>(left: &Spanned<T>, right: &Spanned<U>) -> Span {
    Span::new(left.span.left.clone(), right.span.right.clone())
}

pub(crate) fn typ_span(typ: &Typ) -> &Span {
    match typ {
        Typ::Plain(plain_typ) => &plain_typ.span,
        Typ::Notation(not_typ) => &not_typ.span,
    }
}

pub(crate) fn infix_typ(left: Typ, operator: Spanned<Atom>, right: Typ) -> Typ {
    let span = operator.span.clone();
    Typ::Notation(Spanned::new(
        NotTypKind::Infix(Box::new(left), operator, Box::new(right)),
        span,
    ))
}

pub(crate) fn prefix_typ(operator: Spanned<Atom>, right: Typ) -> Typ {
    let span = operator.span.clone();
    let left = Typ::Notation(Spanned::new(NotTypKind::Seq(vec![]), span.clone()));
    Typ::Notation(Spanned::new(
        NotTypKind::Infix(Box::new(left), operator, Box::new(right)),
        span,
    ))
}

pub(crate) fn infix_exp(left: Exp, operator: Spanned<Atom>, right: Exp) -> Exp {
    let span = span_between(&left, &right);
    Spanned::new(
        ExpKind::Infix(Box::new(left), operator, Box::new(right)),
        span,
    )
}

pub(crate) fn prefix_exp(operator: Spanned<Atom>, right: Exp) -> Exp {
    let span = Span::new(operator.span.left.clone(), right.span.right.clone());
    let left = Spanned::new(ExpKind::Seq(vec![]), operator.span.clone());
    Spanned::new(
        ExpKind::Infix(Box::new(left), operator, Box::new(right)),
        span,
    )
}

pub(crate) fn dot_exp(
    context: &ParserContext,
    base: Exp,
    field: Spanned<Atom>,
    left: ParserLocation,
    right: ParserLocation,
) -> Exp {
    match (base.node, field.node) {
        (ExpKind::Atom(atom), Atom::Keyword(suffix)) => match atom.node {
            Atom::Keyword(prefix) => spanned(
                context,
                ExpKind::Atom(spanned(
                    context,
                    Atom::Keyword(format!("{prefix}.{suffix}")),
                    left,
                    right,
                )),
                left,
                right,
            ),
            node => spanned(
                context,
                ExpKind::Dot(
                    Box::new(Spanned::new(
                        ExpKind::Atom(Spanned::new(node, atom.span)),
                        base.span,
                    )),
                    Spanned::new(Atom::Keyword(suffix), field.span),
                ),
                left,
                right,
            ),
        },
        (node, field_node) => spanned(
            context,
            ExpKind::Dot(
                Box::new(Spanned::new(node, base.span)),
                Spanned::new(field_node, field.span),
            ),
            left,
            right,
        ),
    }
}

pub(crate) enum ExpPostfix {
    Idx(Exp, ParserLocation),
    Slice(Exp, Exp, ParserLocation),
    Upd(Path, Exp, ParserLocation),
    Iter(Iter, ParserLocation),
    Dot(Spanned<Atom>, ParserLocation),
}

pub(crate) fn apply_exp_postfixes(
    context: &ParserContext,
    left: ParserLocation,
    mut exp: Exp,
    postfixes: Vec<ExpPostfix>,
) -> Exp {
    for postfix in postfixes {
        exp = match postfix {
            ExpPostfix::Idx(index, right) => spanned(
                context,
                ExpKind::Idx(Box::new(exp), Box::new(index)),
                left,
                right,
            ),
            ExpPostfix::Slice(start, end, right) => spanned(
                context,
                ExpKind::Slice(Box::new(exp), Box::new(start), Box::new(end)),
                left,
                right,
            ),
            ExpPostfix::Upd(path, value, right) => spanned(
                context,
                ExpKind::Upd(Box::new(exp), path, Box::new(value)),
                left,
                right,
            ),
            ExpPostfix::Iter(iter, right) => {
                spanned(context, ExpKind::Iter(Box::new(exp), iter), left, right)
            }
            ExpPostfix::Dot(atom, right) => dot_exp(context, exp, atom, left, right),
        };
    }
    exp
}

pub(crate) fn def_typ_kind(
    cases: Vec<(Typ, Vec<Hint>)>,
    empty_kind: SyntaxErrorKind,
) -> Result<DefTypKind, SyntaxErrorKind> {
    if cases.is_empty() {
        return Err(empty_kind);
    }
    if cases
        .iter()
        .any(|(typ, hints)| matches!(typ, Typ::Plain(_)) && !hints.is_empty())
    {
        return Err(SyntaxErrorKind::HintsInPlainTypeDefinition);
    }
    if let [(Typ::Plain(plain_typ), _)] = cases.as_slice() {
        Ok(DefTypKind::Plain(plain_typ.clone()))
    } else {
        Ok(DefTypKind::Variant(cases))
    }
}

pub(crate) enum PathStep {
    Idx(Box<Exp>, ParserLocation),
    Slice(Box<Exp>, Box<Exp>, ParserLocation),
    Dot(Spanned<Atom>, ParserLocation),
}

pub(crate) fn build_path(
    context: &ParserContext,
    left: ParserLocation,
    steps: Vec<PathStep>,
) -> Path {
    let mut path = spanned(context, PathKind::Root, left, left);
    for step in steps {
        let (kind, right) = match step {
            PathStep::Idx(index, right) => (PathKind::Idx(Box::new(path), index), right),
            PathStep::Slice(start, end, right) => {
                (PathKind::Slice(Box::new(path), start, end), right)
            }
            PathStep::Dot(field, right) => (PathKind::Dot(Box::new(path), field), right),
        };
        path = spanned(context, kind, left, right);
    }
    path
}

pub(crate) fn syntax_error(
    context: &ParserContext,
    kind: SyntaxErrorKind,
    left: ParserLocation,
    right: ParserLocation,
) -> lalrpop_util::ParseError<ParserLocation, Token, FrontendError> {
    lalrpop_util::ParseError::User {
        error: SyntaxError::new(kind, context.span(left, right)).into(),
    }
}

pub(crate) fn parser_tokens<I>(context: &ParserContext, lexemes: I) -> ParserTokens<'_, I>
where
    I: Iterator,
{
    ParserTokens {
        context,
        lexemes: lexemes.peekable(),
        previous_right: None,
        previous_token: None,
        pending: None,
    }
}

pub(crate) struct ParserTokens<'context, I: Iterator> {
    context: &'context ParserContext,
    lexemes: Peekable<I>,
    previous_right: Option<Position>,
    previous_token: Option<Token>,
    pending: Option<Spanned<Token>>,
}

fn starts_expression(token: &Token) -> bool {
    matches!(
        token,
        Token::TagUpperId(_)
            | Token::Operator(_)
            | Token::TickLeftParen
            | Token::TickLeftBracket
            | Token::TickLeftBrace
            | Token::TickLeftAngle
            | Token::Dot
            | Token::DoubleDot
            | Token::TripleDot
            | Token::Semicolon
            | Token::Backslash
            | Token::Arrow
            | Token::ArrowSub
            | Token::DoubleArrowSub
            | Token::DoubleArrowLong
            | Token::Dollar
            | Token::Tilde
            | Token::Plus
            | Token::Minus
            | Token::Bar
            | Token::Bool
            | Token::Nat
            | Token::Int
            | Token::Text
            | Token::Latex
            | Token::Epsilon
            | Token::BoolLiteral(_)
            | Token::NaturalLiteral(_)
            | Token::HexLiteral(_)
            | Token::TextLiteral(_)
            | Token::UpperId(_)
            | Token::LowerId(_)
            | Token::UpperIdLeftParen(_)
            | Token::LowerIdLeftParen(_)
            | Token::LeftParen
            | Token::LeftBrace
            | Token::Hole
            | Token::NumberedHole(_)
            | Token::MultipleHole
            | Token::EmptyHole
    )
}

fn starts_sequence(token: &Token) -> bool {
    matches!(
        token,
        Token::TagUpperId(_)
            | Token::Operator(_)
            | Token::TickLeftParen
            | Token::TickLeftBracket
            | Token::TickLeftBrace
            | Token::TickLeftAngle
            | Token::Dollar
            | Token::DoubleHash
            | Token::LeftParen
            | Token::LeftBrace
            | Token::Hole
            | Token::NumberedHole(_)
            | Token::MultipleHole
            | Token::EmptyHole
            | Token::Latex
            | Token::Bool
            | Token::Nat
            | Token::Int
            | Token::Text
            | Token::Epsilon
            | Token::BoolLiteral(_)
            | Token::NaturalLiteral(_)
            | Token::HexLiteral(_)
            | Token::TextLiteral(_)
            | Token::UpperId(_)
            | Token::LowerId(_)
            | Token::UpperIdLeftParen(_)
    )
}

fn ends_sequence(token: &Token) -> bool {
    matches!(
        token,
        Token::TagUpperId(_)
            | Token::Operator(_)
            | Token::TickRightParen
            | Token::TickRightBracket
            | Token::TickRightBrace
            | Token::TickRightAngle
            | Token::RightParen
            | Token::RightBracket
            | Token::RightBrace
            | Token::Question
            | Token::IterStar
            | Token::Epsilon
            | Token::Bool
            | Token::Nat
            | Token::Int
            | Token::Text
            | Token::BoolLiteral(_)
            | Token::NaturalLiteral(_)
            | Token::HexLiteral(_)
            | Token::TextLiteral(_)
            | Token::UpperId(_)
            | Token::LowerId(_)
            | Token::DotId(_)
            | Token::Hole
            | Token::NumberedHole(_)
            | Token::MultipleHole
            | Token::EmptyHole
    )
}

impl<I> Iterator for ParserTokens<'_, I>
where
    I: Iterator<Item = Result<Spanned<Token>, LexError>>,
{
    type Item = Result<(ParserLocation, Token, ParserLocation), FrontendError>;

    fn next(&mut self) -> Option<Self::Item> {
        let mut lexeme = match self.pending.take() {
            Some(lexeme) => lexeme,
            None => match self.lexemes.next()? {
                Ok(lexeme) => lexeme,
                Err(error) => return Some(Err(error.into())),
            },
        };

        if lexeme.node == Token::Star {
            let next = self
                .lexemes
                .peek()
                .and_then(|result| result.as_ref().ok())
                .map(|next| &next.node);
            if !next.is_some_and(starts_expression) {
                lexeme.node = Token::IterStar;
            }
        }

        if self.previous_token.as_ref().is_some_and(ends_sequence) && starts_sequence(&lexeme.node)
        {
            let left_position = self
                .previous_right
                .clone()
                .expect("previous token position");
            let right_position = lexeme.span.left.clone();
            self.pending = Some(lexeme);
            self.previous_token = Some(Token::Sequence);
            self.previous_right = Some(right_position.clone());
            return Some(Ok((
                self.context.intern_position(left_position),
                Token::Sequence,
                self.context.intern_position(right_position),
            )));
        }

        let left = self.context.intern_position(lexeme.span.left);
        self.previous_right = Some(lexeme.span.right.clone());
        let right = self.context.intern_position(lexeme.span.right);
        self.previous_token = Some(lexeme.node.clone());
        Some(Ok((left, lexeme.node, right)))
    }
}
