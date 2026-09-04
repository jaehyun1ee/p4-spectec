use p4spec_rust::{
    lang::{
        common::source::{Position, Span},
        data::value::make,
    },
    phrase,
    runtime::{envs::interp::VEnv, var::Variable},
};

fn id(name: &str, line: i64) -> p4spec_rust::lang::il::ast::Id {
    let span = Span::new(
        Position::new("vars.watsup", line, 0),
        Position::new("vars.watsup", line, 1),
    );
    phrase!(node: name.to_owned(), span: span)
}

#[test]
fn test_value_environment_iterates_deterministically_and_replaces_equivalent_keys() {
    let mut venv = VEnv::new();
    venv.insert(
        Variable::new(id("z", 1), vec![]),
        make::bool(false, Span::default()),
    );
    venv.insert(
        Variable::new(id("a", 2), vec![]),
        make::bool(true, Span::default()),
    );
    venv.insert(
        Variable::new(id("a", 8), vec![]),
        make::bool(false, Span::default()),
    );

    let names = venv
        .keys()
        .map(|variable| variable.id.node.as_str())
        .collect::<Vec<_>>();
    assert_eq!(names, ["a", "z"]);
    assert_eq!(venv.len(), 2);
}
