use p4spec_rust::{
    lang::common::source::{Position, Span},
    phrase,
    runner::{BuiltinInterface, Interface, InterfaceErrorKind, NullInterface},
    runtime::value::get,
};

fn id(name: &str) -> p4spec_rust::lang::il::ast::Id {
    phrase!(node: name.to_owned(), span: Span::default())
}

#[test]
fn test_null_interface_reports_configuration_failure() {
    let error = NullInterface
        .call_builtin(&id("sum_int"), &[], &[])
        .unwrap_err();

    assert!(matches!(error.kind, InterfaceErrorKind::NotConfigured));
}

#[test]
fn test_builtin_interface_reports_side_effects_and_clears() {
    let mut interface = BuiltinInterface::new();
    let (value, side_effected) = interface
        .call_builtin(&id("fresh_typeId"), &[], &[])
        .unwrap();

    assert_eq!(get::text(&value), Ok("FRESH__0"));
    assert!(side_effected);

    interface.clear();
    let (value, side_effected) = interface
        .call_builtin(&id("fresh_typeId"), &[], &[])
        .unwrap();
    assert_eq!(get::text(&value), Ok("FRESH__0"));
    assert!(side_effected);
}

#[test]
fn test_builtin_interface_locates_builtin_failures_at_the_call() {
    let span = Span::new(
        Position::new("test.spec", 3, 4),
        Position::new("test.spec", 3, 11),
    );
    let id = phrase!(node: "sum_int".to_owned(), span: span.clone());
    let error = BuiltinInterface::new()
        .call_builtin(&id, &[], &[])
        .unwrap_err();

    assert_eq!(error.span, span);
    assert!(matches!(error.kind, InterfaceErrorKind::Builtin(_)));
}
