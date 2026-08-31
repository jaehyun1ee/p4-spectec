use p4spec_rust::{
    interface::p4_unparse::{P4UnparseError, P4Unparser},
    lang::{common::source::Span, xl::num::Natural},
    runtime::{types::typ, value::make},
};

#[test]
fn unparses_scalar_and_container_values() {
    let unparser = P4Unparser::new();
    let span = Span::default();
    assert_eq!(
        unparser.render(&make::bool(true, span.clone())).unwrap(),
        "true"
    );
    assert_eq!(
        unparser
            .render(&make::nat(Natural::from(42_u64), span.clone()))
            .unwrap(),
        "42"
    );
    assert_eq!(
        unparser
            .render(&make::text("a\n\"b".into(), span.clone()))
            .unwrap(),
        "a\\n\\\"b"
    );

    let tuple_type = typ::tuple(vec![typ::bool(), typ::nat()]);
    let tuple = make::tuple(
        &tuple_type,
        vec![
            make::bool(false, span.clone()),
            make::nat(7_u64.into(), span.clone()),
        ],
        span,
    );
    assert_eq!(unparser.render(&tuple).unwrap(), "(false, 7)");
}

#[test]
fn unsupported_values_return_typed_errors() {
    let structure = make::structure(&typ::bool(), Vec::new(), Span::default());
    assert_eq!(
        P4Unparser::new().render(&structure),
        Err(P4UnparseError::UnsupportedValue("Struct"))
    );
}
