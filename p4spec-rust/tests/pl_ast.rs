use p4spec_rust::{
    domain::source::{HasSpan, Region, Spanned, phrase_list_region},
    lang::{hints::alter, il, pl},
};

fn span(name: &str) -> Region {
    Region::for_file(name)
}

fn id(name: &str) -> il::ast::Id {
    Spanned::new(name.to_owned(), span(name))
}

#[test]
fn annotation_wrappers_forward_source_and_keep_nested_hints() {
    let nested = pl::annot::T {
        node: pl::ast::ExpNode {
            kind: pl::ast::ExpKind::VarE(id("nested")),
            ty: il::ast::TypKind::BoolT,
            span: span("nested-source"),
        },
        hints: pl::annot::Hints {
            prose: Some(alter::T::TextH("nested prose".to_owned())),
            ..pl::annot::empty()
        },
    };
    let outer = pl::annot::no_hints(pl::ast::ExpNode {
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
    assert_eq!(outer.span(), &span("outer-source"));
    assert_eq!(inner.span(), &span("nested-source"));
    assert!(inner.hints.prose.is_some());
    assert_eq!(phrase_list_region(&[outer]), span("outer-source"));
}
