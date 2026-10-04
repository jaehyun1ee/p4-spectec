use p4spectec::lang::{
    common::{
        prim::num::Number,
        source::{FileId, Position, Span},
    },
    data::{
        typ,
        value::{Arena, ValueKind, make},
    },
};
use std::rc::Rc;

#[test]
fn borrowed_primitives_keep_owned_handles_for_both_insertion_orders() {
    let file = FileId::intern("primitive-test.watsup");
    let span = Span::new(Position::new(file, 6, 1), Position::new(file, 6, 5));
    let nums = [
        Number::Nat(0_u64.into()),
        Number::Nat(17_u64.into()),
        Number::Int(17.into()),
        Number::Int((-5).into()),
        Number::Int(
            num_bigint::BigInt::parse_bytes(b"123456789012345678901234567890", 10).unwrap(),
        ),
    ];
    for borrowed_first in [false, true] {
        let nums = nums.clone();
        std::thread::spawn(move || {
            let mut arena = Arena::new();
            let mut arena_reference = Arena::new();
            for round in 0..3 {
                for num in &nums {
                    let span = if round == 1 { span } else { Span::default() };
                    let value = if (round == 0) == borrowed_first {
                        make::num_ref(&mut arena, num, span)
                    } else {
                        make::num(&mut arena, num.clone(), span)
                    }
                    .unwrap();
                    let value_reference =
                        make::num(&mut arena_reference, num.clone(), span).unwrap();
                    assert_eq!(value, value_reference);
                    assert_eq!(arena.canon_id(&value), arena_reference.canon_id(&value_reference));
                    let typ = arena.typ(&value).as_ref().clone();
                    let value_fresh = make::new(
                        &mut arena,
                        ValueKind::Num(num.clone()),
                        Rc::new(typ.clone()),
                        span,
                    )
                    .unwrap();
                    let value_fresh_reference = make::new(
                        &mut arena_reference,
                        ValueKind::Num(num.clone()),
                        Rc::new(typ),
                        span,
                    )
                    .unwrap();
                    assert_eq!(value_fresh, value_fresh_reference);
                    assert_ne!(value_fresh.note, value.note);
                    let alloc = |arena: &mut Arena| {
                        make::tuple(arena, typ::make::tuple(vec![]).node.into(), vec![], span)
                            .unwrap()
                    };
                    assert_eq!(alloc(&mut arena), alloc(&mut arena_reference));
                }
                for text in ["", "alpha", "βeta", "alpha\0tail"] {
                    let span = if round == 1 { span } else { Span::default() };
                    let value = if (round == 0) == borrowed_first {
                        make::text_ref(&mut arena, text, span)
                    } else {
                        make::text(&mut arena, text.into(), span)
                    }
                    .unwrap();
                    let value_reference =
                        make::text(&mut arena_reference, text.into(), span).unwrap();
                    assert_eq!(value, value_reference);
                    let value_fresh = make::new(
                        &mut arena,
                        ValueKind::Text(text.into()),
                        Rc::new(typ::TypKind::Text),
                        span,
                    )
                    .unwrap();
                    let value_fresh_reference = make::new(
                        &mut arena_reference,
                        ValueKind::Text(text.into()),
                        Rc::new(typ::TypKind::Text),
                        span,
                    )
                    .unwrap();
                    assert_eq!(value_fresh, value_fresh_reference);
                    assert_ne!(value_fresh.note, value.note);
                }
            }
        })
        .join()
        .unwrap();
    }
}
