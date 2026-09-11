//! Value constructor annotation tests

use super::span;
use p4spec_rust::lang::{
    data::{
        typ,
        value::{ValueArena, get, make},
    },
    xl::num::{Natural, Number},
};

#[test]
fn test_constructors_preserve_runtime_type_and_span() {
    let mut arena = ValueArena::new();
    let value_span = span("program.p4", 4);
    let value = make::num(
        &mut arena,
        Number::Nat(Natural::from(7_u64)),
        value_span.clone(),
    )
    .unwrap();

    assert_eq!(arena.span(&value), &value_span);
    assert_eq!(arena.typ(&value), &typ::make::nat().node);
    assert_eq!(
        get::num(&arena, &value),
        Ok(&Number::Nat(Natural::from(7_u64)))
    );
}
