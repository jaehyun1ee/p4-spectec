use super::*;

#[test]
fn test_fields_hints_require_text_and_exact_arity() {
    let single = exp(ExpKind::Text("left".to_owned()));
    let sequence = exp(ExpKind::Seq(vec![
        exp(ExpKind::Text("left".to_owned())),
        exp(ExpKind::Text("right".to_owned())),
    ]));

    assert_eq!(
        fields_impl::init(&single),
        Some(FieldHint::new(vec!["left".to_owned()]))
    );
    assert_eq!(
        fields_impl::init(&sequence),
        Some(FieldHint::new(vec!["left".to_owned(), "right".to_owned()]))
    );
    assert_eq!(fields_impl::init(&exp(ExpKind::Hole(Hole::Next))), None);
    let fields = FieldHint::new(vec!["left".to_owned()]);
    assert_eq!(fields_impl::validate(&fields, 1), Ok(()));
    assert_eq!(
        fields_impl::validate(&fields, 2),
        Err(FieldError::ArityMismatch {
            expected: 2,
            actual: 1,
        })
    );
}
