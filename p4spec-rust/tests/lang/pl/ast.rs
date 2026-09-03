use p4spec_rust::{
    lang::common::{
        ds::set::IdSet,
        source::{Position, Span},
    },
    lang::{hints::alter, il, pl, traits::free::Free},
};

fn span(name: &str) -> Span {
    Span::new(Position::new(name, 0, 0), Position::new(name, 0, 0))
}

#[test]
fn test_prose_nodes_collect_free_identifiers_through_annotations() {
    let exp_l = pl::annot::Annotated {
        node: p4spec_rust::note_phrase! {
            node: pl::ast::ExpKind::Var(id("left")),
            note: il::ast::TypKind::Bool,
            span: span("left"),
        },
        hints: pl::annot::Hints::default(),
    };
    let exp_r = pl::annot::Annotated {
        node: p4spec_rust::note_phrase! {
            node: pl::ast::ExpKind::Var(id("right")),
            note: il::ast::TypKind::Bool,
            span: span("right"),
        },
        hints: pl::annot::Hints::default(),
    };
    let expression: pl::ast::Exp = pl::annot::Annotated {
        node: p4spec_rust::note_phrase! { node: pl::ast::ExpKind::Bin(
        il::ast::BinOp::Bool(p4spec_rust::lang::xl::bool::BinOp::And),
        il::ast::OpTyp::Bool,
        Box::new(exp_l),
        Box::new(exp_r),
            ), note: il::ast::TypKind::Bool, span: span("binary") },
        hints: pl::annot::Hints::default(),
    };

    assert_eq!(expression.free(), IdSet::from([id("left"), id("right")]));
}

fn id(name: &str) -> il::ast::Id {
    p4spec_rust::phrase! {
        node: name.to_owned(),
        span: span(name),
    }
}

#[test]
fn test_annotation_wrappers_forward_source_and_keep_nested_hints() {
    let mut nested = pl::annot::Annotated {
        node: p4spec_rust::note_phrase! {
            node: pl::ast::ExpKind::Var(id("nested")),
            note: il::ast::TypKind::Bool,
            span: span("nested-source"),
        },
        hints: pl::annot::Hints::default(),
    };
    nested.hints.prose = Some(alter::AlterationHint::Text("nested prose".to_owned()));
    let outer: pl::ast::Exp = pl::annot::Annotated {
        node: p4spec_rust::note_phrase! { node: pl::ast::ExpKind::Un(
        il::ast::UnOp::Bool(p4spec_rust::lang::xl::bool::UnOp::Not),
        il::ast::OpTyp::Bool,
        Box::new(nested),
            ), note: il::ast::TypKind::Bool, span: span("outer-source") },
        hints: pl::annot::Hints::default(),
    };

    let pl::ast::ExpKind::Un(_, _, inner) = &outer.node.node else {
        panic!("expected nested unary expression")
    };
    assert_eq!(outer.node.span, span("outer-source"));
    assert_eq!(inner.node.span, span("nested-source"));
    assert!(inner.hints.prose.is_some());
    assert_eq!(Span::over(&[outer.node.span]), span("outer-source"));
}
