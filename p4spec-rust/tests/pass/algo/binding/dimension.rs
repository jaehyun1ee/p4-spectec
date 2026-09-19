use super::super::*;

#[test]
fn test_dimension_inference_keeps_the_minimal_occurrence() {
    let exp_id = id_exp("x", 2);
    let iterated = exp(
        ast::ExpKind::Iter(Box::new(exp_id), (ast::Iter::List, vec![])),
        ast::TypKind::Iter(Box::new(typ::make::bool()), ast::Iter::List),
        3,
    );
    let direct = id_exp("x", 4);
    let tuple = exp(ast::ExpKind::Tuple(vec![iterated, direct]), ast::TypKind::Tuple(vec![]), 1);

    let dimensions = dimension::infer_exp(&tuple);
    let (stored_id, actual) = dimensions.iter().next().expect("inferred variable");

    assert_eq!(stored_id.span, span(4));
    let expected = Dim::new(typ::make::bool(), vec![]);
    assert!(actual.sub(&expected));
    assert!(expected.sub(actual));
}
