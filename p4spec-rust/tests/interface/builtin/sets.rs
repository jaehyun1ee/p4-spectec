use p4spec_rust::lang::data::value::ValueArena;
use p4spec_rust::{
    interface::builtin::sets,
    lang::{
        common::{
            notation::mixfix::Mixfix,
            source::{Position, Span},
        },
        data::{
            typ,
            value::{get, make},
        },
    },
};

#[test]
fn test_set_union_retains_parsed_punctuation_spans() {
    let mut arena = ValueArena::new();
    let typ_key = typ::make::bool();
    let typ_set = typ::make::var(super::id("set"), vec![typ_key.clone()]);
    let typ_sets = typ::make::list(typ_set);
    let sets = make::list(&mut arena, &typ_sets, Vec::new(), Span::default()).unwrap();
    let result = sets::unions_set(&mut arena, &[typ_key], &[sets]).unwrap();
    let Mixfix::Brack(left, _, right) = get::case(&arena, &result).unwrap() else {
        panic!("expected set notation");
    };
    assert_eq!(
        *arena.location(left.span),
        Span::new(Position::new("", 1, 0), Position::new("", 1, 2))
    );
    assert_eq!(
        *arena.location(right.span),
        Span::new(Position::new("", 1, 5), Position::new("", 1, 7))
    );
}

#[test]
fn test_set_union_orders_content_and_deduplicates_full_annotations() {
    let mut arena = ValueArena::new();
    let typ_key = typ::make::text();
    let typ_list = typ::make::list(typ_key.clone());
    let typ_set = typ::make::var(super::id("set"), vec![typ_key.clone()]);
    let last = make::text(&mut arena, "z".to_owned(), Span::default()).unwrap();
    let first = make::text(&mut arena, "a".to_owned(), Span::default()).unwrap();
    let equal = make::text(&mut arena, "a".to_owned(), Span::default()).unwrap();
    let distinct = arena
        .relocate(
            equal,
            Span::new(Position::new("key.p4", 1, 0), Position::new("key.p4", 1, 1)),
        )
        .unwrap();
    let elements = make::list(
        &mut arena,
        &typ_list,
        vec![last, first, equal, distinct],
        Span::default(),
    )
    .unwrap();
    let shape = p4spec_rust::frontend::parse::parse_mixop("`{ k `}").unwrap();
    let set = make::case_(
        &mut arena,
        &typ_set,
        p4spec_rust::lang::common::notation::mixop::Mixop::fill(&shape, [elements]).unwrap(),
        Span::default(),
    )
    .unwrap();
    let result = sets::union_set(&mut arena, &[typ_key], &[set, set]).unwrap();
    let elements = get::case(&arena, &result).unwrap().args()[0];
    let values = get::list(&arena, elements).unwrap();
    assert_eq!(values.len(), 3);
    assert!(arena.equal(&values[0], &first));
    assert!(arena.equal(&values[1], &distinct));
    assert!(arena.equal(&values[2], &last));
}
