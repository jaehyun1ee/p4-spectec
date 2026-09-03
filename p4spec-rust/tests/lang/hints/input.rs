use super::*;

#[test]
fn test_input_hints_validate_and_preserve_split_order() {
    let sequence = exp(ExpKind::Seq(vec![
        exp(ExpKind::Hole(Hole::Num(2))),
        exp(ExpKind::Hole(Hole::Num(0))),
    ]));
    assert_eq!(
        input_impl::init(&sequence),
        Some(InputHint::new(vec![2, 0]))
    );
    assert_eq!(
        input_impl::validate(&InputHint::new(vec![]), 3),
        Err(InputError::Empty)
    );
    assert_eq!(
        input_impl::validate(&InputHint::new(vec![1, 1]), 3),
        Err(InputError::DuplicateIndex(1))
    );
    assert_eq!(
        input_impl::validate(&InputHint::new(vec![-1]), 3),
        Err(InputError::IndexOutOfBounds {
            index: -1,
            arity: 3,
        })
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
    assert_eq!(
        input_impl::combine(&hint, items_input, items_output),
        Ok(items.to_vec())
    );
    assert_eq!(
        input_impl::combine(&hint, vec!["zero"], vec!["one", "three"]),
        Err(InputError::InputCountMismatch {
            expected: 2,
            actual: 1,
        })
    );
    assert_eq!(
        input_impl::split(&InputHint::new(vec![4]), items.to_vec()),
        Err(InputError::IndexOutOfBounds { index: 4, arity: 4 })
    );
    assert_eq!(
        input_impl::is_conditional(&InputHint::new(vec![0, 1]), &["left", "right"]),
        Ok(true)
    );
    assert_eq!(
        input_impl::is_conditional(&InputHint::new(vec![0]), &["left", "right"]),
        Ok(false)
    );
}
