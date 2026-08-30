use p4spec_rust::{
    lang::common::source::Span,
    phrase,
    runner::{BuiltinInterface, Interface, InterfaceErrorKind, NullInterface},
};

fn id(name: &str) -> p4spec_rust::lang::il::ast::Id {
    phrase!(node: name.to_owned(), span: Span::default())
}

#[test]
fn null_interface_reports_configuration_failure() {
    let error = NullInterface
        .call_builtin(&mut |_| {}, &id("sum_int"), &[], &[])
        .unwrap_err();

    assert!(matches!(error.kind, InterfaceErrorKind::NotConfigured));
}

#[test]
fn builtin_interface_exposes_checkpoint_and_clear() {
    let mut interface = BuiltinInterface::new();
    let before = interface.checkpoint();
    interface
        .call_builtin(&mut |_| {}, &id("fresh_typeId"), &[], &[])
        .unwrap();
    let after = interface.checkpoint();

    assert!(interface.side_effected(before, after));
    interface.clear();
    assert_eq!(interface.checkpoint(), before);
}
