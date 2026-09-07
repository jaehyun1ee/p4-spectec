use p4spec_rust::{
    lang::common::{
        ds::set::IdSet,
        source::{Position, Span},
    },
    lang::{il, pl, traits::free::Free},
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
