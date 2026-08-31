use super::*;

#[test]
fn test_hint_modules_format_exactly() {
    assert_eq!(
        input_impl::to_string(&InputHint::new(vec![2, 0])),
        "hint(input %2 %0)"
    );
    assert!(InputHint::new(vec![2, 0]).syntax_eq(&InputHint::new(vec![2, 0])));
    assert!(!InputHint::new(vec![2]).syntax_eq(&InputHint::new(vec![0])));
    assert_eq!(
        fields_impl::to_string(&FieldHint::new(vec!["left".into(), "right".into()])),
        "hint(fields left right)"
    );
    assert_eq!(flag_impl::to_string(true), "hint(flag)");
    assert_eq!(flag_impl::to_string(false), "");
    assert_eq!(hint_impl::to_string(&exp(ExpKind::Hole(Hole::Next))), "%");
}
