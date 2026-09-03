use super::super::*;

#[test]
fn test_binding_union_keeps_the_first_span_and_marks_repetition() {
    let id_first = id("x", 1);
    let id_second = id("x", 2);
    let typ_first = typ::bool();
    let dim = Dim::new(typ_first.clone(), vec![]);
    let mut typ_second = typ::bool();
    typ_second.span = span(3);
    let benv_l = BEnv::singleton(id_first.clone(), typ_first);
    let benv_r = BEnv::singleton(id_second, typ_second);

    let benv = benv_l.union(benv_r).expect("equivalent dimensions");

    assert_eq!(benv.iter().next().map(|(id, _)| id), Some(&id_first));
    let binding = benv
        .iter()
        .find(|(id, _)| id.node == id_first.node)
        .map(|(_, binding)| binding)
        .expect("merged binding");
    let Binding::Multiple(actual) = binding else {
        panic!("expected a repeated binding");
    };
    assert!(actual.sub(&dim));
    assert!(dim.sub(actual));
}

#[test]
fn test_binding_union_rejects_conflicting_dimensions_at_the_first_key() {
    let id_first = id("x", 4);
    let id_second = id("x", 8);
    let benv_l = BEnv::singleton(id_first.clone(), typ::bool());
    let benv_r = BEnv::singleton(id_second, typ::bool()).add_iter(ast::Iter::List);

    let error = benv_l.union(benv_r).expect_err("conflicting dimensions");

    assert_eq!(error.kind, AlgoErrorKind::InconsistentDimensions);
    assert_eq!(error.span, id_first.span);
}
