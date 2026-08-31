use super::super::*;

#[test]
fn test_binding_union_keeps_the_first_span_and_marks_repetition() {
    let id_first = id("x", 1);
    let id_second = id("x", 2);
    let dim = Dim::new(typ::bool(), vec![]);
    let mut typ_second = typ::bool();
    typ_second.span = span(3);
    let mut bindings_l = Bindings::new();
    bindings_l.insert(id_first.clone(), Binding::Single(dim.clone()));
    let mut bindings_r = Bindings::new();
    bindings_r.insert(id_second, Binding::Single(Dim::new(typ_second, vec![])));

    let bindings = bind::union(bindings_l, bindings_r).expect("equivalent dimensions");

    assert_eq!(bindings.keys().next(), Some(&id_first));
    let Binding::Multiple(actual) = bindings.get(&id_first).expect("merged binding") else {
        panic!("expected a repeated binding");
    };
    assert!(actual.sub(&dim));
    assert!(dim.sub(actual));
}

#[test]
fn test_binding_union_rejects_conflicting_dimensions_at_the_first_key() {
    let id_first = id("x", 4);
    let id_second = id("x", 8);
    let mut bindings_l = Bindings::new();
    bindings_l.insert(
        id_first.clone(),
        Binding::Single(Dim::new(typ::bool(), vec![])),
    );
    let mut bindings_r = Bindings::new();
    bindings_r.insert(
        id_second,
        Binding::Single(Dim::new(typ::bool(), vec![ast::Iter::List])),
    );

    let error = bind::union(bindings_l, bindings_r).expect_err("conflicting dimensions");

    assert_eq!(error.kind, AlgoErrorKind::InconsistentDimensions);
    assert_eq!(error.span, id_first.span);
}
