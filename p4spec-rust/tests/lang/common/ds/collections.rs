use crate::common::*;

#[test]
fn test_id_set_uses_identifier_text_as_its_key() {
    let id_first = id("x", "first");
    let id_second = id("x", "second");
    let mut ids = IdSet::new();

    assert!(ids.insert(id_first.clone()));
    assert!(!ids.insert(id_second.clone()));
    assert!(ids.contains(&id_second));
    assert_eq!(ids.iter().collect::<Vec<_>>(), vec![&id_first]);
}
#[test]
fn test_id_map_uses_identifier_text_as_its_key() {
    let id_first = id("x", "first");
    let id_second = id("x", "second");
    let mut ids = IdMap::new();

    assert_eq!(ids.insert(id_first.clone(), 1), None);
    assert_eq!(ids.insert(id_second.clone(), 2), Some(1));
    assert_eq!(ids.get(&id_second), Some(&2));
    assert_eq!(ids.keys().collect::<Vec<_>>(), vec![&id_first]);
}
#[test]
fn test_id_map_rejects_mismatched_lists() {
    let keys = [id("x", "first")];
    let values = [1, 2];

    let error = IdMap::from_lists(&keys, &values).expect_err("mismatched list lengths");

    assert_eq!(error, ArityMismatch::new(1, 2));
}
#[test]
fn test_free_identifier_sets_preserve_source_spans() {
    let id_stored = id("x", "stored");
    let id_lookup = id("x", "lookup");
    let exp: il::ast::Exp = p4spec_rust::note_phrase! {
        node: il::ast::ExpKind::Var(id_stored.clone()),
        note: il::ast::TypKind::Bool,
        span: Span::default(),
    };

    let ids: IdSet = exp.free();

    assert_eq!(ids.get(&id_lookup), Some(&id_stored));
}

#[test]
fn test_free_into_extends_one_ordered_set_without_duplicates() {
    let variable = |name| -> il::ast::Exp {
        p4spec_rust::note_phrase! {
            node: il::ast::ExpKind::Var(id(name, name)),
            note: il::ast::TypKind::Bool,
            span: Span::default(),
        }
    };
    let exp: il::ast::Exp = p4spec_rust::note_phrase! {
        node: il::ast::ExpKind::Tuple(vec![variable("x"), variable("x"), variable("y")]),
        note: il::ast::TypKind::Bool,
        span: Span::default(),
    };
    let mut ids = IdSet::from([id("seed", "seed")]);

    exp.free_into(&mut ids);

    assert_eq!(
        ids,
        IdSet::from([id("seed", "seed"), id("x", "x"), id("y", "y")])
    );
}
