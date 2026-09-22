use super::*;

#[test]
fn test_input_hints_validate_and_preserve_split_order() {
    let sequence =
        exp(ExpKind::Seq(vec![exp(ExpKind::Hole(Hole::Num(2))), exp(ExpKind::Hole(Hole::Num(0)))]));
    assert_eq!(input_impl::init(&sequence), Some(InputHint::new(vec![2, 0])));
    assert_eq!(input_impl::validate(&InputHint::new(vec![]), 3), Err(InputError::Empty));
    assert_eq!(
        input_impl::validate(&InputHint::new(vec![1, 1]), 3),
        Err(InputError::DuplicateIndex(1))
    );
    assert_eq!(
        input_impl::validate(&InputHint::new(vec![3]), 3),
        Err(InputError::IndexOutOfBounds { index: 3, arity: 3 })
    );
    let hint = InputHint::new(vec![2, 0]);
    assert_eq!(input_impl::validate(&hint, 3), Ok(()));

    let items = ["zero", "one", "two", "three"];
    let (items_input, items_output) = input_impl::split(&hint, items.to_vec()).unwrap();
    assert_eq!(items_input, vec!["zero", "two"]);
    assert_eq!(items_output, vec!["one", "three"]);
    assert_eq!(input_impl::combine(&hint, items_input, items_output), Ok(items.to_vec()));
    assert_eq!(
        input_impl::combine(&hint, vec!["zero"], vec!["one", "three"]),
        Err(InputError::InputCountMismatch { expected: 2, actual: 1 })
    );
    assert_eq!(
        input_impl::split(&InputHint::new(vec![4]), items.to_vec()),
        Err(InputError::IndexOutOfBounds { index: 4, arity: 4 })
    );
    assert_eq!(
        input_impl::is_conditional(&InputHint::new(vec![0, 1]), &["left", "right"]),
        Ok(true)
    );
    assert_eq!(input_impl::is_conditional(&InputHint::new(vec![0]), &["left", "right"]), Ok(false));
}

#[test]
fn test_zero_arity_default_hint_supports_operations_but_not_source_validation() {
    let hint = InputHint::new(vec![]);
    assert_eq!(input_impl::validate(&hint, 0), Err(InputError::Empty));
    assert_eq!(input_impl::split::<()>(&hint, vec![]), Ok((vec![], vec![])));
    assert_eq!(input_impl::combine::<()>(&hint, vec![], vec![]), Ok(vec![]));
    assert_eq!(input_impl::is_conditional::<()>(&hint, &[]), Ok(true));
    assert_eq!(input_impl::split(&hint, vec![0]), Err(InputError::Empty));
}

#[test]
fn test_input_hint_duplicates_take_precedence_over_bounds() {
    let hint = InputHint::new(vec![9, 0, 0]);
    assert_eq!(input_impl::validate(&hint, 2), Err(InputError::DuplicateIndex(0)));
}

#[test]
fn test_input_hint_preserves_element_spans_without_changing_equivalence() {
    let mut exp_a = exp(ExpKind::Hole(Hole::Num(2)));
    let mut exp_b = exp(ExpKind::Hole(Hole::Num(0)));
    exp_a.span = Span::new(Position::new("hint", 1, 11), Position::new("hint", 1, 13));
    exp_b.span = Span::new(Position::new("hint", 1, 14), Position::new("hint", 1, 16));
    let exp_hint = exp(ExpKind::Seq(vec![exp_a.clone(), exp_b.clone()]));
    let hint = input_impl::init(&exp_hint).unwrap();
    assert_eq!(hint.indices()[0].span, exp_a.span);
    assert_eq!(hint.indices()[1].span, exp_b.span);
    assert_eq!(hint, InputHint::new(vec![2, 0]));
    assert_eq!(InputHint::new(vec![2, 0]).indices()[0].span, Span::default());
    assert_eq!(
        hint.into_indices()
            .iter()
            .map(|idx| idx.node)
            .collect::<Vec<_>>(),
        vec![2, 0]
    );

    let mut exp_repeated = exp_a.clone();
    exp_repeated.span = exp_b.span.clone();
    let hint_repeated = input_impl::init(&exp(ExpKind::Seq(vec![exp_a, exp_repeated]))).unwrap();
    assert_eq!(input_impl::validate(&hint_repeated, 3), Err(InputError::DuplicateIndex(2)));
}
