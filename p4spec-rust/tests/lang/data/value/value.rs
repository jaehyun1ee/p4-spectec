//! Shared value data tests

use super::{hash, span};
use p4spec_rust::{
    lang::{
        common::source::Span,
        data::{
            typ,
            value::{ValueArena, ValueKind, make},
        },
        traits::{cmp::SyntaxCmp, eq::SyntaxEq},
    },
    wire::ocaml::lang::il::ValueCodec,
    yojson::ExternalData,
};

#[test]
fn test_value_equality_includes_type_and_source() {
    let mut arena = ValueArena::new();
    let bool_value = make::bool(&mut arena, true, span("a.p4", 1)).unwrap();
    let different_type = make::new(
        &mut arena,
        ValueKind::Bool(true),
        typ::make::text().node.clone().into(),
        span("a.p4", 1),
    )
    .unwrap();
    let different_source = make::bool(&mut arena, true, span("b.p4", 9)).unwrap();

    assert_ne!(bool_value, different_type);
    assert_ne!(bool_value, different_source);
}

#[test]
fn test_external_data_has_the_same_total_order_and_hash_for_signed_zero() {
    let negative_zero = ExternalData::Float(-0.0);
    let positive_zero = ExternalData::Float(0.0);

    assert_eq!(negative_zero.cmp(&positive_zero), std::cmp::Ordering::Equal);
    assert_eq!(hash(&negative_zero), hash(&positive_zero));
}

#[test]
fn test_copied_values_share_interned_storage() {
    let mut arena = ValueArena::new();
    let value = make::bool(&mut arena, true, Span::default()).unwrap();
    let cloned = value;

    assert!((value == cloned));
}

#[test]
fn test_runtime_values_are_il_ast_values() {
    let mut arena = ValueArena::new();
    let value: p4spec_rust::lang::il::ast::Value =
        make::bool(&mut arena, true, Span::default()).unwrap();

    assert_eq!(arena.to_string(&value), "true");
    assert!(ValueCodec::encode(&arena, &value).is_ok());
}

#[test]
fn test_value_views_resolve_nested_handles_in_each_arena() {
    let mut arena_l = ValueArena::new();
    let mut arena_r = ValueArena::new();
    let child_l = make::bool(&mut arena_l, true, span("left.p4", 1)).unwrap();
    let child_r_false = make::bool(&mut arena_r, false, span("right.p4", 2)).unwrap();
    let child_r = make::bool(&mut arena_r, true, span("right.p4", 3)).unwrap();
    let child_r = arena_r
        .update_typ(child_r, typ::TypKind::Text.into())
        .unwrap();
    let value_l = make::tuple(
        &mut arena_l,
        typ::TypKind::Bool.into(),
        vec![child_l],
        span("left.p4", 4),
    )
    .unwrap();
    let value_r_false = make::tuple(
        &mut arena_r,
        typ::TypKind::Bool.into(),
        vec![child_r_false],
        span("right.p4", 5),
    )
    .unwrap();
    let value_r = make::tuple(
        &mut arena_r,
        typ::TypKind::Text.into(),
        vec![child_r],
        span("right.p4", 6),
    )
    .unwrap();

    assert!(arena_l.view(value_l).syntax_eq(&arena_r.view(value_r)));
    assert!(
        arena_l
            .view(value_l)
            .syntax_cmp(&arena_r.view(value_r))
            .is_eq()
    );
    assert!(
        !arena_l
            .view(value_l)
            .syntax_eq(&arena_r.view(value_r_false))
    );
    assert!(
        arena_l
            .view(value_l)
            .syntax_cmp(&arena_r.view(value_r_false))
            .is_gt()
    );
    assert!(
        arena_r
            .view(value_r_false)
            .syntax_cmp(&arena_l.view(value_l))
            .is_lt()
    );
}
