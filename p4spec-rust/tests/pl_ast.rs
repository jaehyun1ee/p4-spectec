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
    let mut nested = pl::ast::exp(
        pl::ast::ExpKind::Var(id("nested")),
        il::ast::TypKind::Bool,
        span("nested-source"),
    );
    nested.hints.prose = Some(alter::AlterationHint::Text("nested prose".to_owned()));
    let outer = pl::ast::exp(
        pl::ast::ExpKind::Un(
            il::ast::UnOp::Bool(p4spec_rust::lang::xl::bool::UnOp::Not),
            il::ast::OpTyp::Bool,
            Box::new(nested),
        ),
        il::ast::TypKind::Bool,
        span("outer-source"),
    );

    let pl::ast::ExpKind::Un(_, _, inner) = &outer.node.node.kind else {
        panic!("expected nested unary expression")
    };
    assert_eq!(outer.node.span, span("outer-source"));
    assert_eq!(inner.node.span, span("nested-source"));
    assert!(inner.hints.prose.is_some());
    assert_eq!(Span::over(&[outer.node.span]), span("outer-source"));
}
