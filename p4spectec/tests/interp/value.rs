use std::rc::Rc;

use p4spectec::lang::{
    common::source::{FileId, Position, Span},
    data::{
        typ::{self, TypKind},
        value::{Arena, Value, ValueKind, make},
    },
};

/// Captures the shared boolean type without allocating in either compared arena.
fn bool_typ() -> Rc<TypKind> {
    let mut arena = Arena::new();
    let value = make::bool(&mut arena, false, Span::default()).unwrap();
    arena.typ(&value).clone()
}

/// Compares every allocated handle against the ordinary interning path.
fn check_sequence(arena: &mut Arena, arena_reference: &mut Arena, first: bool) {
    let typ = bool_typ();
    let file = FileId::intern("bool-test.watsup");
    let span = Span::new(Position::new(file, 3, 1), Position::new(file, 3, 2));
    let span_file = Span::new(Position::new(file, 0, 0), Position::new(file, 0, 0));
    // Mix both polarity orders and custom spans around repeated generated values
    for (idx, (value, span)) in [
        (first, span),
        (first, Span::default()),
        (!first, Span::default()),
        (first, Span::default()),
        (!first, span_file),
        (!first, Span::default()),
    ]
    .into_iter()
    .enumerate()
    {
        let value_a = make::bool(arena, value, span).unwrap();
        let value_b =
            make::new(arena_reference, ValueKind::Bool(value), typ.clone(), span).unwrap();
        assert_eq!(value_a, value_b);
        // Caller-owned equal types must still receive distinct fresh note handles
        let value_a_fresh =
            make::new(arena, ValueKind::Bool(value), Rc::new(TypKind::Bool), span).unwrap();
        let value_b_fresh =
            make::new(arena_reference, ValueKind::Bool(value), Rc::new(TypKind::Bool), span)
                .unwrap();
        assert_eq!(value_a_fresh, value_b_fresh);
        assert_ne!(value_a_fresh.note, value_a.note);
        assert_eq!(value_a_fresh.node, value_a.node);
        // Later allocations expose any hidden change to interner numbering
        let alloc = |arena: &mut Arena, value: Value| {
            let value_num = make::nat(arena, (idx as u64).into(), Span::default()).unwrap();
            make::tuple(
                arena,
                typ::make::tuple(vec![typ::make::bool(), typ::make::nat()])
                    .node
                    .into(),
                vec![value, value_num],
                span,
            )
            .unwrap()
        };
        assert_eq!(alloc(arena, value_a), alloc(arena_reference, value_b));
    }
}

#[test]
fn repeated_booleans_preserve_exact_handles_and_fresh_types() {
    for first in [false, true] {
        check_sequence(&mut Arena::new(), &mut Arena::new(), first);
    }
}

#[test]
fn resetting_a_runner_discards_its_boolean_handles() {
    let mut runner = super::support::sl_runner("dec $value() : bool\ndef $value() = true");
    check_sequence(runner.arena_mut(), &mut Arena::new(), false);
    runner.reset();
    check_sequence(runner.arena_mut(), &mut Arena::new(), true);
}
