use super::super::*;

#[test]
fn test_shallow_cases_accept_only_iterated_variables_as_arguments() {
    let variable = var_exp("x", 1);
    let iterated = exp(
        ast::ExpKind::Iter(Box::new(variable), (ast::Iter::List, vec![])),
        ast::TypKind::Iter(Box::new(typ::make::bool()), ast::Iter::List),
        1,
    );
    let shallow_case = exp(
        ast::ExpKind::Case(Box::new(Mixfix::Arg(iterated))),
        ast::TypKind::Bool,
        1,
    );
    let nested_tuple = exp(
        ast::ExpKind::Tuple(vec![var_exp("x", 2)]),
        ast::TypKind::Tuple(vec![typ::make::bool()]),
        2,
    );
    let deep_case = exp(
        ast::ExpKind::Case(Box::new(Mixfix::Arg(nested_tuple))),
        ast::TypKind::Bool,
        2,
    );

    assert!(shallow::check_exp(&shallow_case));
    assert!(!shallow::check_exp(&deep_case));
}
