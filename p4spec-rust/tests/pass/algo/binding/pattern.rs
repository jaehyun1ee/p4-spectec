use super::super::*;

#[test]
fn test_pattern_overlap_requires_intersection_in_every_dimension() {
    let owner_span = span(1);
    let pattern_a = [pattern_set(&["A", "B"]), pattern_set(&["X"])]
        .into_iter()
        .collect::<PatternSets>();
    let pattern_b = [pattern_set(&["B"]), pattern_set(&["X", "Y"])]
        .into_iter()
        .collect::<PatternSets>();
    let pattern_c = [pattern_set(&["B"]), pattern_set(&["Y"])]
        .into_iter()
        .collect::<PatternSets>();

    assert!(pattern::has_overlap(&owner_span, &pattern_a, &pattern_b).expect("matching arity"));
    assert!(!pattern::has_overlap(&owner_span, &pattern_a, &pattern_c).expect("matching arity"));
}

#[test]
fn test_pattern_arity_errors_use_the_owning_source_span() {
    let owner_span = span(31);
    let patterns_l = [pattern_set(&["A"])].into_iter().collect::<PatternSets>();
    let patterns_r = [pattern_set(&["A"]), pattern_set(&["B"])]
        .into_iter()
        .collect::<PatternSets>();

    let error = pattern::has_overlap(&owner_span, &patterns_l, &patterns_r)
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
fn test_pattern_sets_order_mixfix_structure_before_rendered_text() {
    let argument = crate::phrase! { node: Mixfix::Arg(typ::bool()), span:  span(2) };
    let atom = not_typ("A", 1);
    let patterns: PatternSet = [atom, argument].into_iter().collect();
    let ordered = patterns.iter().collect::<Vec<_>>();

    assert!(matches!(ordered[0].node, Mixfix::Arg(_)));
    assert!(matches!(ordered[1].node, Mixfix::Atom(_)));
}

#[test]
fn test_pattern_subtraction_preserves_cartesian_fragment_order() {
    let owner_span = span(1);
    let total = [pattern_set(&["A", "B"]), pattern_set(&["X", "Y"])]
        .into_iter()
        .collect::<PatternSets>();
    let covered = [pattern_set(&["A"]), pattern_set(&["X"])]
        .into_iter()
        .collect::<PatternSets>();

    let missing = pattern::subtract(&owner_span, &total, &covered).expect("matching arity");

    assert_eq!(
        missing,
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
