use std::{
    collections::{BTreeSet, hash_map::DefaultHasher},
    hash::{Hash, Hasher},
    rc::Rc,
};

use num_bigint::BigInt;
use p4spec_rust::wire::ocaml::lang::il::ValueCodec;
use p4spec_rust::{
    lang::{
        common::source::{Position, Span},
        traits::print::Print,
        xl::num::{Natural, Number},
    },
    runtime::{
        types::typ,
        value::{Fresh, ValueError, ValueKind, ValueTag, get, make},
    },
    yojson::ExternalData,
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
fn test_semantic_equality_ignores_type_and_source_but_preserves_kind() {
    let bool_a = make::bool(true, span("a.p4", 1));
    let bool_b = make::new(ValueKind::Bool(true), typ::text().node, span("b.p4", 9));
    let text = make::text("true".to_owned(), Span::default());

    assert_eq!(bool_a, bool_b);
    assert_eq!(hash(&bool_a), hash(&bool_b));
    assert_ne!(bool_a, text);
}

#[test]
fn test_value_order_matches_ocaml_variant_and_payload_order() {
    let bool_value = make::bool(false, Span::default());
    let nat_value = make::nat(Natural::from(0_u64), Span::default());
    let int_value = make::int(BigInt::from(0), Span::default());
    let text_value = make::text(String::new(), Span::default());

    assert!(bool_value < nat_value);
    assert!(nat_value < int_value);
    assert!(int_value < text_value);
}

#[test]
fn test_function_values_have_a_total_order_by_semantic_id() {
    let id_a = p4spec_rust::phrase!(node: "a".to_owned(), span: span("a.spec", 1));
    let id_b = p4spec_rust::phrase!(node: "b".to_owned(), span: span("b.spec", 2));
    let func_a = make::func(id_a, Vec::new(), Vec::new(), typ::bool(), Span::default());
    let func_b = make::func(id_b, Vec::new(), Vec::new(), typ::bool(), Span::default());

    assert!(func_a < func_b);
    assert_ne!(func_a, func_b);
    assert_eq!(BTreeSet::from([func_a, func_b]).len(), 2);
}

#[test]
fn test_external_float_order_matches_ocaml_for_signed_zero() {
    let typ = typ::text();
    let negative_zero = make::external(&typ, ExternalData::Float(-0.0), Span::default());
    let positive_zero = make::external(&typ, ExternalData::Float(0.0), Span::default());

    assert_eq!(negative_zero, positive_zero);
    assert_eq!(hash(&negative_zero), hash(&positive_zero));
    assert_eq!(BTreeSet::from([negative_zero, positive_zero]).len(), 1);
}

#[test]
fn test_constructors_preserve_runtime_type_and_span() {
    let value_span = span("program.p4", 4);
    let value = make::num(Number::Nat(Natural::from(7_u64)), value_span.clone());

    assert_eq!(value.span, value_span);
    assert_eq!(value.note.typ, typ::nat().node);
    assert_eq!(get::num(&value), Ok(&Number::Nat(Natural::from(7_u64))));
}

#[test]
fn test_getters_report_expected_and_actual_kinds() {
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
fn test_cloned_values_share_immutable_storage() {
    let value = make::bool(true, Span::default());
    let cloned = Rc::clone(&value);

    assert!(Rc::ptr_eq(&value, &cloned));
}

#[test]
fn test_fresh_type_variables_are_isolated_by_instance() {
    let mut first = Fresh::new();
    let mut second = Fresh::new();

    let first_initial = first.fresh().0;
    let first_next = first.fresh().0;
    let second_initial = second.fresh().0;

    assert_eq!(first_initial, second_initial);
    assert_ne!(first_initial, first_next);
}

#[test]
fn test_runtime_values_are_il_ast_values() {
    let value: p4spec_rust::lang::il::ast::Value = make::bool(true, Span::default());

    assert_eq!(value.to_string(), "true");
    assert!(ValueCodec::encode(&value).is_ok());
}
