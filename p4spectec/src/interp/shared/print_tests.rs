use std::rc::Rc;

use super::prepare::{Prepare, PrepareContext};
use crate::{
    lang::{
        common::{notation::atom::Atom, source::Span},
        data::{
            notation::{Mixfix, MixopArena},
            typ::TypKind,
        },
        il::ast as il,
        pl::{annot::Annotated, ast as pl},
        traits::print::Print,
    },
    runtime::envs::interp::shared::frame::FrameLayout,
};

fn exp_il(node: il::ExpKind) -> il::Exp {
    crate::note_phrase!(node: node, note: Rc::new(TypKind::Bool), span: Span::default())
}

fn exp_pl(node: pl::ExpKind) -> pl::Exp {
    Annotated::new(crate::note_phrase!(node: node, note: TypKind::Bool, span: Span::default()))
}

#[test]
fn prepared_il_prints_nested_cases_with_its_arena() {
    let exp_inner = exp_il(il::ExpKind::Case(Box::new(Mixfix::infix(
        Mixfix::arg(exp_il(il::ExpKind::Bool(true))),
        crate::phrase!(node: Atom::Keyword("OP".to_owned()), span: Span::default()),
        Mixfix::arg(exp_il(il::ExpKind::Bool(false))),
    ))));
    let exp = exp_il(il::ExpKind::Case(Box::new(Mixfix::brack(
        crate::phrase!(node: Atom::LParen, span: Span::default()),
        Mixfix::arg(exp_inner),
        crate::phrase!(node: Atom::RParen, span: Span::default()),
    ))));
    assert_eq!(Print::to_string(&exp), "`( true OP false `)");
    let mut arena_mixop = MixopArena::new();
    let mut layout = FrameLayout::default();
    let exp =
        exp.prepare(&mut PrepareContext { layout: &mut layout, arena_mixop: &mut arena_mixop });
    assert_eq!(super::print::exp_to_string(&arena_mixop, &exp), "`( true OP false `)");
    let arg = crate::phrase!(node: super::prepare::ast::ArgKind::Exp(Box::new(exp)), span: Span::default());
    assert_eq!(super::print::arg_to_string(&arena_mixop, &arg), "`( true OP false `)");
}

#[test]
fn prepared_pl_preserves_its_distinct_case_parentheses() {
    let exp_inner = exp_pl(pl::ExpKind::Case(Box::new(Mixfix::infix(
        Mixfix::arg(exp_pl(pl::ExpKind::Bool(true))),
        crate::phrase!(node: Atom::Keyword("OP".to_owned()), span: Span::default()),
        Mixfix::arg(exp_pl(pl::ExpKind::Bool(false))),
    ))));
    let exp = exp_pl(pl::ExpKind::Case(Box::new(Mixfix::brack(
        crate::phrase!(node: Atom::LParen, span: Span::default()),
        Mixfix::arg(exp_inner),
        crate::phrase!(node: Atom::RParen, span: Span::default()),
    ))));
    assert_eq!(Print::to_string(&exp), "(`( (true OP false) `))");
    let mut arena_mixop = MixopArena::new();
    let mut layout = FrameLayout::default();
    let exp =
        exp.prepare(&mut PrepareContext { layout: &mut layout, arena_mixop: &mut arena_mixop });
    assert_eq!(
        crate::interp::pl::print::exp_to_string(&arena_mixop, &exp),
        "(`( (true OP false) `))"
    );
}
