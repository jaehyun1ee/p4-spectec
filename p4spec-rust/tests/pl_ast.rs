use p4spec_rust::{
    domain::source::{Position, Span, Spanned},
    lang::{hints::alter, il, pl},
};

fn span(name: &str) -> Span {
    Span::new(Position::new(name, 0, 0), Position::new(name, 0, 0))
}

fn id(name: &str) -> il::ast::Id {
    Spanned::new(name.to_owned(), span(name))
}

#[test]
fn annotation_wrappers_forward_source_and_keep_nested_hints() {
    let nested = pl::annot::Annotated {
        node: pl::ast::ExpNode {
            kind: pl::ast::ExpKind::VarE(id("nested")),
            ty: il::ast::TypKind::BoolT,
            span: span("nested-source"),
        },
        hints: pl::annot::Hints {
            prose: Some(alter::AlterationHint::TextH("nested prose".to_owned())),
            ..pl::annot::Hints::default()
        },
    };
    let outer = pl::annot::Annotated::new(pl::ast::ExpNode {
        kind: pl::ast::ExpKind::UnE(
            il::ast::UnOp::NotOp,
            il::ast::OpTyp::BoolT,
            Box::new(nested),
        ),
        ty: il::ast::TypKind::BoolT,
        span: span("outer-source"),
    });

    let pl::ast::ExpKind::UnE(_, _, inner) = &outer.node.kind else {
        panic!("expected nested unary expression")
    };
    assert_eq!(outer.node.span, span("outer-source"));
    assert_eq!(inner.node.span, span("nested-source"));
    assert!(inner.hints.prose.is_some());
    assert_eq!(Span::over(&[outer.node.span]), span("outer-source"));
}
