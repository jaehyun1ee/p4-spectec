use p4spec_rust::{
    lang::common::source::{Position, Span, Spanned},
    lang::common::{
        Id,
        ds::{
            map::{ArityMismatch, IdMap},
            set::IdSet,
        },
        noted::Noted,
    },
    lang::{il, traits::free::Free},
};

fn id(name: &str, file: &str) -> Id {
    Spanned::new(
        name.to_owned(),
        Span::new(Position::new(file, 0, 0), Position::new(file, 0, 0)),
    )
}

#[test]
fn id_set_uses_identifier_text_as_its_key() {
    let id_first = id("x", "first");
    let id_second = id("x", "second");
    let mut ids = IdSet::new();

    assert!(ids.insert(id_first.clone()));
    assert!(!ids.insert(id_second.clone()));
    assert!(ids.contains(&id_second));
    assert_eq!(ids.iter().collect::<Vec<_>>(), vec![&id_first]);
}

#[test]
fn id_map_uses_identifier_text_as_its_key() {
    let id_first = id("x", "first");
    let id_second = id("x", "second");
    let mut ids = IdMap::new();

    assert_eq!(ids.insert(id_first.clone(), 1), None);
    assert_eq!(ids.insert(id_second.clone(), 2), Some(1));
    assert_eq!(ids.get(&id_second), Some(&2));
    assert_eq!(ids.keys().collect::<Vec<_>>(), vec![&id_first]);
}

#[test]
fn id_map_rejects_mismatched_lists() {
    let keys = [id("x", "first")];
    let values = [1, 2];

    let error = IdMap::from_lists(&keys, &values).expect_err("mismatched list lengths");

    assert_eq!(error, ArityMismatch::new(1, 2));
}

#[test]
fn free_identifier_sets_preserve_source_spans() {
    let id_stored = id("x", "stored");
    let id_lookup = id("x", "lookup");
    let exp = p4spec_rust::spanned! {
        node: Noted {
            kind: il::ast::ExpKind::Var(id_stored.clone()),
            note: il::ast::TypKind::Bool,
        },
        span: Span::default(),
    };

    let ids: IdSet = exp.free();

    assert_eq!(ids.get(&id_lookup), Some(&id_stored));
}

#[test]
fn fresh_variables_lookup_aliases_by_identifier_text() {
    let span_requested = Span::new(
        Position::new("requested", 0, 0),
        Position::new("requested", 0, 0),
    );
    let typ = Spanned::new(il::ast::TypKind::Bool, Span::default());
    let mut metavars = IdMap::new();
    metavars.insert(id("Alias", "declaration"), typ.clone());

    let var = il::fresh::var_from_typ(&metavars, &IdSet::new(), span_requested.clone(), &typ);

    assert_eq!(var.id.node, "Alias");
    assert_eq!(var.id.span, span_requested);
}
