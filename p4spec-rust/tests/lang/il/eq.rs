use p4spec_rust::{
    lang::{
        common::source::{Position, Span},
        traits::eq::SyntaxEq,
    },
    runtime::{types::typ, value::make},
};

fn span(file: &str) -> Span {
    let left = Position::new(file, 1, 0);
    let right = Position::new(file, 1, 1);
    Span::new(left, right)
}

#[test]
fn test_function_value_syntax_equality_ignores_identifier_spans() {
    let id_l = p4spec_rust::phrase!(node: "f".to_owned(), span: span("left.spec"));
    let id_r = p4spec_rust::phrase!(node: "f".to_owned(), span: span("right.spec"));
    let value_l = make::func(id_l, Vec::new(), Vec::new(), typ::bool(), Span::default());
    let value_r = make::func(id_r, Vec::new(), Vec::new(), typ::bool(), Span::default());

    assert!(value_l.syntax_eq(&value_r));
}
