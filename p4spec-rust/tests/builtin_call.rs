use num_bigint::BigInt;
use p4spec_rust::{
    domain::source::{Region, Spanned},
    interface::builtin::{BuiltinResult, call::Builtins},
    lang::il::ast::Typ,
    runtime::{
        r#type::typ::make as make_type,
        value::{ValueRef, get, make},
    },
};

fn span(file: &str) -> Region {
    Region::for_file(file)
}

fn id(name: &str, file: &str) -> Spanned<String> {
    Spanned::new(name.to_owned(), span(file))
}

fn override_sum(
    add: &mut dyn FnMut(ValueRef),
    _span: &Region,
    _type_args: &[Typ],
    _values: &[ValueRef],
) -> BuiltinResult {
    let value = make::text("override".to_owned(), Region::none());
    add(value.clone());
    Ok(value)
}

#[test]
fn fresh_type_id_advances_checkpoint_and_init_resets_it() {
    let mut builtins = Builtins::new();
    let before = builtins.checkpoint();
    let first = builtins
        .invoke(&mut |_| {}, &id("fresh_typeId", "fresh-call"), &[], &[])
        .unwrap();
    let after = builtins.checkpoint();
    assert_eq!(get::text(&first), Ok("FRESH__0"));
    assert!(Builtins::side_effected(before, after));

    let second = builtins
        .invoke(&mut |_| {}, &id("fresh_typeId", "fresh-call"), &[], &[])
        .unwrap();
    assert_eq!(get::text(&second), Ok("FRESH__1"));

    builtins.init();
    let reset = builtins
        .invoke(&mut |_| {}, &id("fresh_typeId", "fresh-call"), &[], &[])
        .unwrap();
    assert_eq!(get::text(&reset), Ok("FRESH__0"));
}

#[test]
fn pure_dispatch_does_not_change_checkpoint() {
    let mut builtins = Builtins::new();
    let values = make::list(
        &make_type::list_type(make_type::nat_type()),
        vec![
            make::nat(BigInt::from(2), span("two")),
            make::nat(BigInt::from(3), span("three")),
        ],
        span("values"),
    );
    let before = builtins.checkpoint();
    let result = builtins
        .invoke(&mut |_| {}, &id("sum_nat", "sum-call"), &[], &[values])
        .unwrap();
    assert!(matches!(
        get::num(&result).expect("number"),
        p4spec_rust::lang::xl::num::T::Nat(value) if value == &BigInt::from(5)
    ));
    assert!(!Builtins::side_effected(before, builtins.checkpoint()));
}

#[test]
fn extensions_override_base_entries_and_unknown_names_keep_the_id_span() {
    let mut builtins = Builtins::with_extensions([("sum_nat", override_sum)]);
    let result = builtins
        .invoke(&mut |_| {}, &id("sum_nat", "override-call"), &[], &[])
        .unwrap();
    assert_eq!(get::text(&result), Ok("override"));

    let error = builtins
        .invoke(&mut |_| {}, &id("missing", "missing-call"), &[], &[])
        .unwrap_err();
    assert_eq!(error.span, span("missing-call"));
    assert!(
        error
            .message
            .contains("implementation for builtin missing is missing")
    );
}

#[test]
fn fresh_type_id_checks_both_type_and_value_arity() {
    let mut builtins = Builtins::new();
    let error = builtins
        .invoke(
            &mut |_| {},
            &id("fresh_typeId", "arity-call"),
            &[make_type::int_type()],
            &[],
        )
        .unwrap_err();
    assert_eq!(error.span, span("arity-call"));
    assert_eq!(builtins.checkpoint(), 0);
}
