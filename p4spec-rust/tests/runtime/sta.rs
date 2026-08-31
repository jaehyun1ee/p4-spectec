use p4spec_rust::{
    lang::{
        common::source::{Position, Span},
        il::ast::{self, Iter, TypKind},
    },
    runtime::sta::Dim,
};

fn span(file: &str) -> Span {
    Span::new(Position::new(file, 1, 0), Position::new(file, 1, 1))
}

fn typ(kind: TypKind, file: &str) -> ast::Typ {
    p4spec_rust::phrase! {
        node: kind,
        span: span(file),
    }
}

#[test]
fn test_type_dimensions_track_iterator_prefixes() {
    let bool_a = Dim::new(typ(TypKind::Bool, "a"), vec![Iter::Opt]);
    let bool_list = bool_a.clone().add_iter(Iter::List);

    assert!(bool_a.sub(&bool_list));
    assert!(!bool_list.sub(&bool_a));
}
