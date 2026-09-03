use std::cmp::Ordering;

use p4spec_rust::lang::traits::cmp::SyntaxCmp;

use super::*;

fn span(line: i64) -> Span {
    let position = Position::new("cmp", line, 0);
    Span::new(position.clone(), position)
}

fn id_at(name: &str, line: i64) -> ast::Id {
    p4spec_rust::phrase! {
        node: name.to_owned(),
        span: span(line),
    }
}

#[test]
fn test_syntax_comparison_ignores_nested_source_spans() {
    let typ_l = p4spec_rust::phrase! {
        node: ast::TypKind::Var(id_at("T", 1), vec![]),
        span: span(2),
    };
    let typ_r = p4spec_rust::phrase! {
        node: ast::TypKind::Var(id_at("T", 3), vec![]),
        span: span(4),
    };

    assert_eq!(typ_l.syntax_cmp(&typ_r), Ordering::Equal);
}
