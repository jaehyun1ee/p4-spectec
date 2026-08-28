//! AST construction shared by LALRPOP grammar actions
//!
//! Grammar productions use `spanned` and `span_between` to attach source
//! ranges while building EL nodes. Expression builders such as `sequence_exp`,
//! `numeric_bin_exp`, and `compare_exp` keep repeated grammar actions uniform.
//! `apply_exp_postfixes` folds parsed postfixes from left to right, while
//! `try_build_def_typ_kind` performs the semantic checks needed when a type
//! definition becomes an EL node. `Parsed` and `ParsedList` retain right
//! boundaries for syntax that EL intentionally drops.
//!
//! # Examples
//!
//! ```text
//! sequence_exp(sequence_exp(a, b), c) => Seq([a, b, c])
//! apply_exp_postfixes(x, [Idx(i), Dot(f)]) => Dot(Idx(x, i), f)
//! ```

use crate::lang::{
    common::{
        notation::atom::Atom,
        source::{Position, Span, Spanned},
    },
    el::ast::{
        BinOp, CmpOp, DefTypKind, Exp, ExpKind, Hint, Iter, NotTypKind, Path, PathKind, PlainTyp,
        Typ,
    },
    xl,
};

use super::{
    ctx::{ParserContext, ParserLocation},
    error::{FrontendError, SyntaxError, SyntaxErrorKind},
    lexer::Token,
};

pub(crate) struct Parsed<T> {
    pub(crate) node: T,
    pub(crate) right: Position,
}

pub(crate) struct ParsedList<T> {
    pub(crate) nodes: Vec<T>,
    pub(crate) right: Option<Position>,
}

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

pub(crate) fn try_build_def_typ_kind(
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

pub(crate) fn sequence_exp(left: Exp, right: Exp) -> Exp {
    let span = span_between(&left, &right);
    let mut expressions = match left.node {
        ExpKind::Seq(expressions) if expressions.len() >= 2 => expressions,
        node => vec![Spanned::new(node, left.span)],
    };
    expressions.push(right);
    Spanned::new(ExpKind::Seq(expressions), span)
}

pub(crate) fn fuse_exp(left: Exp, right: Exp) -> Exp {
    let span = span_between(&left, &right);
    Spanned::new(ExpKind::Fuse(Box::new(left), Box::new(right)), span)
}

pub(crate) fn numeric_bin_exp(left: Exp, operator: xl::num::BinOp, right: Exp) -> Exp {
    let span = span_between(&left, &right);
    Spanned::new(
        ExpKind::Bin(Box::new(left), BinOp::Num(operator), Box::new(right)),
        span,
    )
}

pub(crate) fn boolean_bin_exp(left: Exp, operator: xl::bool::BinOp, right: Exp) -> Exp {
    let span = span_between(&left, &right);
    Spanned::new(
        ExpKind::Bin(Box::new(left), BinOp::Bool(operator), Box::new(right)),
        span,
    )
}

pub(crate) fn compare_exp(left: Exp, operator: CmpOp, right: Exp) -> Exp {
    let span = span_between(&left, &right);
    Spanned::new(
        ExpKind::Cmp(Box::new(left), operator, Box::new(right)),
        span,
    )
}

pub(crate) fn cat_exp(left: Exp, right: Exp) -> Exp {
    let span = span_between(&left, &right);
    Spanned::new(ExpKind::Cat(Box::new(left), Box::new(right)), span)
}

pub(crate) fn cons_exp(head: Exp, tail: Exp) -> Exp {
    let span = span_between(&head, &tail);
    Spanned::new(ExpKind::Cons(Box::new(head), Box::new(tail)), span)
}

pub(crate) fn member_exp(left: Exp, right: Exp) -> Exp {
    let span = span_between(&left, &right);
    Spanned::new(ExpKind::Mem(Box::new(left), Box::new(right)), span)
}

pub(crate) fn subtype_exp(left: Exp, plain_typ: PlainTyp) -> Exp {
    let span = span_between(&left, &plain_typ);
    Spanned::new(ExpKind::Sub(Box::new(left), plain_typ), span)
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
