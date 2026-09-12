use p4spec_rust::lang::data::value::ValueArena;
use p4spec_rust::{
    frontend::parse::parse_mixop,
    interface::builtin::maps,
    lang::{
        common::{
            notation::{atom::Atom, mixfix::Mixfix, mixop::Mixop},
            source::Span,
        },
        data::{
            typ,
            value::{get, make},
        },
    },
};

#[test]
fn test_map_update_retains_notation() {
    let mut arena = ValueArena::new();
    let span = Span::default();
    let typ_key = typ::make::text();
    let typ_value = typ::make::bool();
    let typ_pair = typ::make::var(super::id("pair"), vec![typ_key.clone(), typ_value.clone()]);
    let typ_pairs = typ::make::list(typ_pair);
    let typ_map = typ::make::var(super::id("map"), vec![typ_key.clone(), typ_value.clone()]);
    let pairs = make::list(
        &mut arena,
        typ_pairs.node.clone().into(),
        Vec::new(),
        span.clone(),
    )
    .unwrap();
    let shape = parse_mixop("`{ k `}").unwrap();
    let map = make::case(
        &mut arena,
        typ_map.node.clone().into(),
        Mixop::fill(&shape, [pairs]).unwrap(),
        span.clone(),
    )
    .unwrap();
    let key = make::text(&mut arena, "key".to_owned(), span.clone()).unwrap();
    let value = make::bool(&mut arena, true, span).unwrap();
    let result = maps::add_map(
        &mut arena,
        &[typ_key.clone(), typ_value.clone()],
        &[map, key, value],
    )
    .unwrap();
    let mut span_key = Span::default();
    span_key.left.line = 17;
    let key_updated = arena.update_span(key, span_key).unwrap();
    let value_updated = make::bool(&mut arena, false, Span::default()).unwrap();
    let result = maps::add_map(
        &mut arena,
        &[typ_key, typ_value],
        &[result, key_updated, value_updated],
    )
    .unwrap();
    let Mixfix::Brack(atom_l, inner, atom_r) = get::case(&arena, &result).unwrap() else {
        panic!("expected map notation");
    };
    assert_eq!(atom_l.node, Atom::LBrace);
    assert_eq!(atom_r.node, Atom::RBrace);
    let Mixfix::Arg(pairs) = inner.as_ref() else {
        panic!("expected map entries")
    };
    let pairs = get::list(&arena, pairs).unwrap();
    assert_eq!(pairs.len(), 1);
    assert_eq!(
        *get::case(&arena, &pairs[0]).unwrap().args()[1],
        value_updated
    );
    let Mixfix::Seq(items) = get::case(&arena, &pairs[0]).unwrap() else {
        panic!("expected pair notation")
    };
    let Mixfix::Atom(colon) = &items[1] else {
        panic!("expected colon")
    };
    assert_eq!(colon.node, Atom::Operator(":".to_owned()));
}
