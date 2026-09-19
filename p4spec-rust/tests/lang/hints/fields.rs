use super::*;

#[test]
fn test_fields_initialize_only_text_and_require_exact_arity() {
    let hint_exp = exp(ExpKind::Seq(vec![
        exp(ExpKind::Text("left".to_owned())),
        exp(ExpKind::Text("right".to_owned())),
    ]));
    let hint = FieldHint::new(vec!["left".to_owned(), "right".to_owned()]);

    assert_eq!(fields_impl::init(&hint_exp), Some(hint.clone()));
    assert_eq!(
        fields_impl::init(&exp(ExpKind::Text("field".to_owned()))),
        Some(FieldHint::new(vec!["field".to_owned()]))
    );
    assert_eq!(fields_impl::init(&exp(ExpKind::Seq(vec![exp(ExpKind::Hole(Hole::Next))]))), None);
    assert_eq!(fields_impl::validate(&hint, 2), Ok(()));
    assert_eq!(
        fields_impl::validate(&hint, 1),
        Err(FieldError::ArityMismatch { expected: 1, actual: 2 })
    );
}
