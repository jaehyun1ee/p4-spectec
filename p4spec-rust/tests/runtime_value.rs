use std::{
    collections::hash_map::DefaultHasher,
    hash::{Hash, Hasher},
    rc::Rc,
};

use num_bigint::BigInt;
use p4spec_rust::{
    lang::{
        common::source::{Position, Span},
        xl::num::{Natural, Number},
    },
    runtime::{
        types::typ,
        value::{Fresh, ValueError, ValueKind, ValueTag, get, make},
    },
};

fn span(file: &str, line: i64) -> Span {
    Span::new(Position::new(file, line, 0), Position::new(file, line, 1))
}

fn hash(value: &impl Hash) -> u64 {
    let mut hasher = DefaultHasher::new();
    value.hash(&mut hasher);
    hasher.finish()
}

#[test]
fn semantic_equality_ignores_type_and_source_but_preserves_kind() {
    let bool_a = make::bool(true, span("a.p4", 1));
    let bool_b = make::new(ValueKind::Bool(true), typ::text().node, span("b.p4", 9));
    let text = make::text("true".to_owned(), Span::default());

    assert_eq!(bool_a, bool_b);
    assert_eq!(hash(&bool_a), hash(&bool_b));
    assert_ne!(bool_a, text);
}

#[test]
fn value_order_matches_ocaml_variant_and_payload_order() {
    let bool_value = make::bool(false, Span::default());
    let nat_value = make::nat(Natural::from(0_u64), Span::default());
    let int_value = make::int(BigInt::from(0), Span::default());
    let text_value = make::text(String::new(), Span::default());

    assert!(bool_value < nat_value);
    assert!(nat_value < int_value);
    assert!(int_value < text_value);
}

#[test]
fn constructors_preserve_runtime_type_and_span() {
    let value_span = span("program.p4", 4);
    let value = make::num(Number::Nat(Natural::from(7_u64)), value_span.clone());

    assert_eq!(value.span, value_span);
    assert_eq!(value.typ, typ::nat().node);
    assert_eq!(get::num(&value), Ok(&Number::Nat(Natural::from(7_u64))));
}

#[test]
fn getters_report_expected_and_actual_kinds() {
    let value = make::text("payload".to_owned(), Span::default());

    assert_eq!(
        get::bool(&value),
        Err(ValueError::UnexpectedKind {
            expected: ValueTag::Bool,
            actual: ValueTag::Text,
        })
    );
    assert_eq!(
        get::one(&[]),
        Err(ValueError::ExpectedCount {
            expected: 1,
            actual: 0,
        })
    );
}

#[test]
fn cloned_values_share_immutable_storage() {
    let value = make::bool(true, Span::default());
    let cloned = Rc::clone(&value);

    assert!(Rc::ptr_eq(&value, &cloned));
}

#[test]
fn fresh_type_variables_are_isolated_by_instance() {
    let mut first = Fresh::new();
    let mut second = Fresh::new();

    assert_eq!(first.fresh().0.node, "__FRESH0");
    assert_eq!(first.fresh().0.node, "__FRESH1");
    assert_eq!(second.fresh().0.node, "__FRESH0");
}
