use p4spec_rust::lang::data::value::ValueArena;
use p4spec_rust::{
    frontend::parse::parse_mixop,
    interface::builtin::maps,
    lang::{
        common::{
            notation::{mixfix::Mixfix, mixop::Mixop},
            source::{Position, Span},
        },
        data::{
            typ,
            value::{get, make},
        },
    },
};

#[test]
fn test_map_update_retains_parsed_punctuation_spans() {
    let mut arena = ValueArena::new();
    let span = Span::default();
    let typ_key = typ::make::text();
    let typ_value = typ::make::bool();
    let typ_pair = typ::make::var(super::id("pair"), vec![typ_key.clone(), typ_value.clone()]);
    let typ_pairs = typ::make::list(typ_pair);
    let typ_map = typ::make::var(super::id("map"), vec![typ_key.clone(), typ_value.clone()]);
    let pairs = make::list(&mut arena, &typ_pairs, Vec::new(), span.clone()).unwrap();
    let shape = parse_mixop("`{ k `}").unwrap();
    let map = make::case_(
        &mut arena,
        &typ_map,
        Mixop::fill(&shape, [pairs]).unwrap(),
        span.clone(),
    )
    .unwrap();
    let key = make::text(&mut arena, "key".to_owned(), span.clone()).unwrap();
    let value = make::bool(&mut arena, true, span).unwrap();
    let result = maps::add_map(&mut arena, &[typ_key, typ_value], &[map, key, value]).unwrap();
    let Mixfix::Brack(left, inner, right) = get::case(&arena, &result).unwrap() else {
        panic!("expected map notation");
    };
    assert_eq!(
        *arena.location(left.span),
        Span::new(Position::new("", 1, 0), Position::new("", 1, 2))
    );
    assert_eq!(
        *arena.location(right.span),
        Span::new(Position::new("", 1, 5), Position::new("", 1, 7))
    );
    let Mixfix::Arg(pairs) = inner.as_ref() else {
        panic!("expected map entries")
    };
    let pairs = get::list(&arena, pairs).unwrap();
    let Mixfix::Seq(items) = get::case(&arena, &pairs[0]).unwrap() else {
        panic!("expected pair notation")
    };
    let Mixfix::Atom(colon) = &items[1] else {
        panic!("expected colon")
    };
    assert_eq!(
        *arena.location(colon.span),
        Span::new(Position::new("", 1, 2), Position::new("", 1, 5))
    );
}

#[test]
fn test_map_lookup_compares_key_content_and_annotations_not_handles() {
    let mut arena = ValueArena::new();
    let typ_key = typ::make::text();
    let typ_value = typ::make::bool();
    let typ_pair = typ::make::var(super::id("pair"), vec![typ_key.clone(), typ_value.clone()]);
    let pairs = make::list(
        &mut arena,
        &typ::make::list(typ_pair),
        Vec::new(),
        Span::default(),
    )
    .unwrap();
    let typ_map = typ::make::var(super::id("map"), vec![typ_key.clone(), typ_value.clone()]);
    let shape = parse_mixop("`{ k `}").unwrap();
    let map = make::case_(
        &mut arena,
        &typ_map,
        Mixop::fill(&shape, [pairs]).unwrap(),
        Span::default(),
    )
    .unwrap();
    let key = make::text(&mut arena, "key".to_owned(), Span::default()).unwrap();
    let value = make::bool(&mut arena, true, Span::default()).unwrap();
    let targs = [typ_key, typ_value];
    let map = maps::add_map(&mut arena, &targs, &[map, key, value]).unwrap();
    let equal_key = make::text(&mut arena, "key".to_owned(), Span::default()).unwrap();
    assert_ne!(key, equal_key);
    let found = maps::find_map(&mut arena, &targs, &[map, equal_key]).unwrap();
    assert_eq!(get::opt(&arena, &found).unwrap().copied(), Some(value));

    let moved_key = arena
        .relocate(
            equal_key,
            Span::new(Position::new("key.p4", 1, 0), Position::new("key.p4", 1, 3)),
        )
        .unwrap();
    let typed_key = arena.annotate(equal_key, typ::make::bool().node).unwrap();
    for distinct_key in [moved_key, typed_key] {
        let found = maps::find_map(&mut arena, &targs, &[map, distinct_key]).unwrap();
        assert_eq!(get::opt(&arena, &found).unwrap().copied(), None);
    }
}
