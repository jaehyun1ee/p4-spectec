use p4spec_rust::{
    lang::{
        common::source::{Position, Span, Spanned},
        il::ast::{self, Iter, TypKind},
    },
    runtime::sta::Dim,
};

fn span(file: &str) -> Span {
    Span::new(Position::new(file, 1, 0), Position::new(file, 1, 1))
}

fn typ(kind: TypKind, file: &str) -> ast::Typ {
    Spanned::new(kind, span(file))
}

#[test]
fn type_dimensions_ignore_spans_and_track_iterator_prefixes() {
    let bool_a = Dim::new(typ(TypKind::Bool, "a"), vec![Iter::Opt]);
    let bool_b = Dim::new(typ(TypKind::Bool, "b"), vec![Iter::Opt]);
    let bool_list = bool_a.clone().add_iter(Iter::List);

    assert!(bool_a.equiv(&bool_b));
    assert!(bool_a.sub(&bool_list));
    assert!(!bool_list.sub(&bool_a));
}
