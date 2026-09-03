use super::super::*;

#[test]
fn test_pattern_overlap_requires_intersection_in_every_dimension() {
    let owner_span = span(1);
    let pattern_sets_a = [pattern_set(&["A", "B"]), pattern_set(&["X"])]
        .into_iter()
        .collect::<PatternSets>();
    let pattern_sets_b = [pattern_set(&["B"]), pattern_set(&["X", "Y"])]
        .into_iter()
        .collect::<PatternSets>();
    let pattern_sets_c = [pattern_set(&["B"]), pattern_set(&["Y"])]
        .into_iter()
        .collect::<PatternSets>();

    assert!(
        pattern::has_overlap(&owner_span, &pattern_sets_a, &pattern_sets_b)
            .expect("matching arity")
    );
    assert!(
        !pattern::has_overlap(&owner_span, &pattern_sets_a, &pattern_sets_c)
            .expect("matching arity")
    );
}

#[test]
fn test_pattern_arity_errors_use_the_owning_source_span() {
    let owner_span = span(31);
    let pattern_sets_l = [pattern_set(&["A"])].into_iter().collect::<PatternSets>();
    let pattern_sets_r = [pattern_set(&["A"]), pattern_set(&["B"])]
        .into_iter()
        .collect::<PatternSets>();

    let error = pattern::has_overlap(&owner_span, &pattern_sets_l, &pattern_sets_r)
        .expect_err("different pattern arities");

    assert_eq!(
        error.kind,
        AlgoErrorKind::PatternArityMismatch {
            expected: 1,
            actual: 2,
        }
    );
    assert_eq!(error.span, owner_span);
}

#[test]
fn test_pattern_sets_ignore_wrapper_spans() {
    let pattern_set_l = [not_typ("A", 1)].into_iter().collect::<PatternSet>();
    let pattern_set_r = [not_typ("A", 2)].into_iter().collect::<PatternSet>();

    assert_eq!(pattern_set_l, pattern_set_r);
}

#[test]
fn test_pattern_sets_ignore_nested_source_spans() {
    let typ_l = crate::phrase! {
        node: ast::TypKind::Var(id("T", 1), vec![]),
        span: span(3),
    };
    let typ_r = crate::phrase! {
        node: ast::TypKind::Var(id("T", 2), vec![]),
        span: span(3),
    };
    let not_typ_l = crate::phrase! { node: Mixfix::Arg(typ_l), span: span(4) };
    let not_typ_r = crate::phrase! { node: Mixfix::Arg(typ_r), span: span(4) };
    let pattern_set_l = [not_typ_l].into_iter().collect::<PatternSet>();
    let pattern_set_r = [not_typ_r].into_iter().collect::<PatternSet>();

    assert_eq!(pattern_set_l, pattern_set_r);
}

#[test]
fn test_pattern_subtraction_preserves_cartesian_fragment_order() {
    let owner_span = span(1);
    let pattern_sets_total = [pattern_set(&["A", "B"]), pattern_set(&["X", "Y"])]
        .into_iter()
        .collect::<PatternSets>();
    let pattern_sets_covered = [pattern_set(&["A"]), pattern_set(&["X"])]
        .into_iter()
        .collect::<PatternSets>();

    let pattern_sets_group_missing =
        pattern::subtract(&owner_span, &pattern_sets_total, &pattern_sets_covered)
            .expect("matching arity");

    assert_eq!(
        pattern_sets_group_missing,
        vec![
            [pattern_set(&["B"]), pattern_set(&["X", "Y"])]
                .into_iter()
                .collect::<PatternSets>(),
            [pattern_set(&["A"]), pattern_set(&["Y"])]
                .into_iter()
                .collect::<PatternSets>(),
        ]
    );
}
