use p4spec_rust::{
    lang::{
        common::source::{Position, Span},
        il::ast::Iter,
    },
    phrase,
    runtime::var::Variable,
};

fn id(name: &str, line: i64) -> p4spec_rust::lang::il::ast::Id {
    let span = Span::new(
        Position::new("vars.watsup", line, 0),
        Position::new("vars.watsup", line, 1),
    );
    phrase!(node: name.to_owned(), span: span)
}

#[test]
fn test_variables_ignore_identifier_spans_and_order_iterators() {
    let plain_a = Variable::new(id("x", 1), vec![]);
    let plain_b = Variable::new(id("x", 9), vec![]);
    let optional = Variable::new(id("x", 2), vec![Iter::Opt]);
    let listed = Variable::new(id("x", 3), vec![Iter::List]);

    assert_eq!(plain_a, plain_b);
    assert!(plain_a < optional);
    assert!(optional < listed);
    assert_eq!(listed.to_string(), "x*");
}
