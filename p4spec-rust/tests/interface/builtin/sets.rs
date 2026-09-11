use p4spec_rust::lang::data::value::ValueArena;
use p4spec_rust::{
    frontend::parse::parse_mixop,
    interface::builtin::sets,
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
fn test_set_union_retains_notation() {
    let mut arena = ValueArena::new();
    let typ_key = typ::make::bool();
    let typ_set = typ::make::var(super::id("set"), vec![typ_key.clone()]);
    let typ_sets = typ::make::list(typ_set);
    let sets = make::list(
        &mut arena,
        typ_sets.node.clone().into(),
        Vec::new(),
        Span::default(),
    )
    .unwrap();
    let result = sets::unions_set(&mut arena, &[typ_key], &[sets]).unwrap();
    let Mixfix::Brack(atom_l, _, atom_r) = get::case(&arena, &result).unwrap() else {
        panic!("expected set notation");
    };
    assert_eq!(atom_l.node, Atom::LBrace);
    assert_eq!(atom_r.node, Atom::RBrace);
}

#[test]
fn test_set_union_deduplicates_annotated_elements_in_syntax_order() {
    let mut arena = ValueArena::new();
    let value_true = make::bool(&mut arena, true, Span::default()).unwrap();
    let mut span = Span::default();
    span.left.line = 17;
    let value_true_updated = arena.update_span(value_true, span).unwrap();
    let value_true_updated = arena
        .update_typ(value_true_updated, typ::TypKind::Text.into())
        .unwrap();
    let value_false = make::bool(&mut arena, false, Span::default()).unwrap();
    let values = make::list(
        &mut arena,
        typ::make::list(typ::make::bool()).node.clone().into(),
        vec![value_true, value_true_updated, value_false],
        Span::default(),
    )
    .unwrap();
    let shape = parse_mixop("`{ k `}").unwrap();
    let value_set = make::case(
        &mut arena,
        typ::make::var(super::id("set"), vec![typ::make::bool()])
            .node
            .clone()
            .into(),
        Mixop::fill(&shape, [values]).unwrap(),
        Span::default(),
    )
    .unwrap();
    let result =
        sets::union_set(&mut arena, &[typ::make::bool()], &[value_set, value_set]).unwrap();
    let values = get::case(&arena, &result).unwrap().args()[0];
    let values = get::list(&arena, values).unwrap();
    assert_eq!(values.len(), 2);
    assert!(!get::bool(&arena, &values[0]).unwrap());
    assert!(get::bool(&arena, &values[1]).unwrap());
}
