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

#[test]
fn test_external_json_order_agrees_with_identity_and_hash_across_arenas() {
    use serde_json::json;
    let payloads = vec![
        json!(null),
        json!(false),
        json!(true),
        json!(i64::MIN),
        json!(-1),
        json!(0),
        json!(1),
        json!(9_007_199_254_740_992_u64),
        json!(9_007_199_254_740_993_u64),
        json!(i64::MAX),
        json!(i64::MAX as u64 + 1),
        json!(u64::MAX),
        json!(-f64::MAX),
        json!(-1.0),
        json!(-0.0),
        json!(0.0),
        json!(f64::from_bits(1)),
        json!(f64::MIN_POSITIVE),
        json!(1.0),
        json!(f64::MAX),
        json!(""),
        json!("a"),
        json!([]),
        json!([null]),
        json!([null, false]),
        json!([false]),
        json!([-0.0]),
        json!([0.0]),
        json!({}),
        json!({"a": null}),
        json!({"a": null, "b": false}),
        json!({"b": false, "a": null}),
        json!({"a": false}),
        json!({"b": null}),
    ];
    let mut arena_a = ValueArena::new();
    let mut arena_b = ValueArena::new();
    make::bool(&mut arena_b, true, Span::default()).unwrap();
    let typ = std::rc::Rc::new(typ::TypKind::Bool);
    let values_a = payloads
        .iter()
        .map(|json| {
            make::external(&mut arena_a, typ.clone(), json.clone(), Span::default()).unwrap()
        })
        .collect::<Vec<_>>();
    let values_b = payloads
        .iter()
        .map(|json| {
            make::external(&mut arena_b, typ.clone(), json.clone(), span("other.p4", 7)).unwrap()
        })
        .collect::<Vec<_>>();
    for (idx_a, value_a) in values_a.iter().enumerate() {
        for (idx_b, value_b) in values_b.iter().enumerate() {
            let order = if payloads[idx_a] == payloads[idx_b] {
                std::cmp::Ordering::Equal
            } else {
                idx_a.cmp(&idx_b)
            };
            assert_eq!(
                arena_a.view(*value_a).syntax_cmp(&arena_b.view(*value_b)),
                order,
                "{idx_a}, {idx_b}"
            );
            assert_eq!(
                arena_b.view(*value_b).syntax_cmp(&arena_a.view(*value_a)),
                order.reverse()
            );
            assert_eq!(
                arena_a.view(*value_a).syntax_eq(&arena_b.view(*value_b)),
                order.is_eq()
            );
            assert_eq!(
                arena_a.canon_id(value_a) == arena_a.canon_id(&values_a[idx_b]),
                order.is_eq()
            );
            assert_eq!(
                arena_a.kind(value_a) == arena_b.kind(value_b),
                payloads[idx_a] == payloads[idx_b]
            );
            if order.is_eq() {
                assert_eq!(hash(arena_a.kind(value_a)), hash(arena_b.kind(value_b)));
            }
        }
    }
}
