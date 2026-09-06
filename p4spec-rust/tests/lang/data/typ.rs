use std::cmp::Ordering;

use p4spec_rust::{
    lang::{
        common::source::{Position, Span},
        data::typ::{Typ, TypKind},
        traits::cmp::SyntaxCmp,
    },
    phrase,
};

fn span(line: i64) -> Span {
    let position = Position::new("cmp", line, 0);
    Span::new(position.clone(), position)
}

fn id_at(name: &str, line: i64) -> p4spec_rust::lang::common::Id {
    phrase! {
        node: name.to_owned(),
        span: span(line),
    }
}

fn typ_at(kind: TypKind, line: i64) -> Typ {
    phrase! {
        node: kind,
        span: span(line),
    }
}

#[test]
fn test_syntax_comparison_ignores_nested_source_spans() {
    let typ_l = typ_at(TypKind::Var(id_at("T", 1), vec![]), 2);
    let typ_r = typ_at(TypKind::Var(id_at("T", 3), vec![]), 4);

    assert_eq!(typ_l.syntax_cmp(&typ_r), Ordering::Equal);
}
