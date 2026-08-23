use p4spec_rust::{
    domain::source::{HasSpan, Region, phrase_list_region},
    lang::{
        hints::{alter, fields},
        pl,
        sl::ast::TypKind,
    },
};

#[test]
fn annotations_forward_nested_source_regions_without_losing_hints() {
    let inner_region = Region::for_file("inner");
    let outer_region = Region::for_file("outer");
    let inner = pl::annot::T {
        node: pl::ast::ExpNode {
            kind: pl::ast::ExpKind::BoolE(true),
            ty: TypKind::BoolT,
            span: inner_region.clone(),
        },
        hints: pl::annot::Hints {
            prose: Some(alter::T::TextH("inner prose".into())),
            prose_fields: Some(fields::T::from(["field".into()])),
            ..pl::annot::empty()
        },
    };
    let outer = pl::annot::no_hints(pl::ast::ExpNode {
        kind: pl::ast::ExpKind::LenE(Box::new(inner)),
        ty: TypKind::BoolT,
        span: outer_region.clone(),
    });

    assert_eq!(outer.span(), &outer_region);
    let pl::ast::ExpKind::LenE(inner) = &outer.node.kind else {
        panic!("expected nested length expression")
    };
    assert_eq!(inner.span(), &inner_region);
    assert_eq!(
        inner.hints.prose,
        Some(alter::T::TextH("inner prose".into()))
    );
    assert_eq!(phrase_list_region(&[outer]), outer_region);
}
