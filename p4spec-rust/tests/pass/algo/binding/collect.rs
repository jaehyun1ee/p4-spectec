use super::super::*;

#[test]
fn test_conversion_propagates_located_binding_errors() {
    let variable = var_exp("x", 41);
    let negated = exp(
        ast::ExpKind::Un(
            ast::UnOp::Bool(xl::bool::UnOp::Not),
            ast::OpTyp::Bool,
            Box::new(variable),
        ),
        ast::TypKind::Bool,
        40,
    );
    let spec = function_spec(
        vec![typ::bool()],
        vec![negated],
        exp(ast::ExpKind::Bool(true), ast::TypKind::Bool, 42),
        vec![],
    );

    let error = algo::convert(spec).expect_err("binding below a unary operator");

    assert_eq!(
        error.kind,
        AlgoErrorKind::NonInvertibleBinding("unary operator")
    );
    assert_eq!(error.span, span(41));
}

#[test]
fn test_collection_rejects_a_binding_inside_a_noninvertible_operator() {
    let variable = var_exp("x", 7);
    let negated = exp(
        ast::ExpKind::Un(
            ast::UnOp::Bool(xl::bool::UnOp::Not),
            ast::OpTyp::Bool,
            Box::new(variable),
        ),
        ast::TypKind::Bool,
        6,
    );

    let error =
        collect::collect_exp(&Context::new(), &negated).expect_err("binding under unary operator");

    assert_eq!(
        error.kind,
        AlgoErrorKind::NonInvertibleBinding("unary operator")
    );
    assert_eq!(error.span, span(7));
}

#[test]
fn test_expression_collection_reports_right_associated_conflict_span() {
    let iterated = exp(
        ast::ExpKind::Iter(Box::new(var_exp("x", 3)), (ast::Iter::List, vec![])),
        ast::TypKind::Iter(Box::new(typ::bool()), ast::Iter::List),
        3,
    );
    let tuple = exp(
        ast::ExpKind::Tuple(vec![var_exp("x", 1), var_exp("x", 2), iterated]),
        ast::TypKind::Tuple(vec![]),
        1,
    );

    let error = collect::collect_exp(&Context::new(), &tuple)
        .expect_err("third occurrence conflicts with the repeated tail binding");

    assert_eq!(error.kind, AlgoErrorKind::InconsistentDimensions);
    assert_eq!(error.span, span(2));
}

#[test]
fn test_argument_collection_reports_right_associated_conflict_span() {
    let iterated = exp(
        ast::ExpKind::Iter(Box::new(var_exp("x", 3)), (ast::Iter::List, vec![])),
        ast::TypKind::Iter(Box::new(typ::bool()), ast::Iter::List),
        3,
    );
    let args = [var_exp("x", 1), var_exp("x", 2), iterated]
        .into_iter()
        .map(|exp| crate::phrase! { node: ast::ArgKind::Exp(Box::new(exp)), span:  span(1) })
        .collect::<Vec<_>>();

    let error = collect::collect_args(&Context::new(), &args)
        .expect_err("third occurrence conflicts with the repeated tail binding");

    assert_eq!(error.kind, AlgoErrorKind::InconsistentDimensions);
    assert_eq!(error.span, span(2));
}
