use num_bigint::BigInt;
use p4spec_rust::{
    domain::{
        atom::Atom,
        mixfix::Mixfix,
        source::{Region, Spanned},
    },
    interface::{BuiltinInterface, Interface, NullInterface, P4Interface},
    lang::{el::ast as el, il::ast as il, sl::ast as sl},
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

fn invoke_through_trait(
    interface: &mut dyn Interface,
    name: &str,
    type_args: &[p4spec_rust::lang::il::ast::Typ],
    values: &[ValueRef],
) -> Result<ValueRef, p4spec_rust::interface::InterfaceError> {
    interface.call_builtin(&mut |_| {}, &id(name, "trait-call"), type_args, values)
}

#[test]
fn builtin_interface_dispatches_and_reports_state_through_the_trait() {
    let mut interface = BuiltinInterface::new();
    let before = interface.checkpoint();
    let fresh = invoke_through_trait(&mut interface, "fresh_typeId", &[], &[]).unwrap();
    assert_eq!(get::text(&fresh), Ok("FRESH__0"));
    assert!(interface.side_effected(before, interface.checkpoint()));

    interface.clear();
    let reset = invoke_through_trait(&mut interface, "fresh_typeId", &[], &[]).unwrap();
    assert_eq!(get::text(&reset), Ok("FRESH__0"));
}

#[test]
fn p4_print_uses_the_supplied_unparser_and_records_the_text_value() {
    let mut interface = P4Interface::new(|value| match get::num(value).unwrap() {
        p4spec_rust::lang::xl::num::T::Nat(value) | p4spec_rust::lang::xl::num::T::Int(value) => {
            format!("p4<{value}>")
        }
    });
    let input = make::int(BigInt::from(42), span("input"));
    let mut recorded = Vec::new();
    let result = interface
        .call_builtin(
            &mut |value| recorded.push(value),
            &id("print_", "print-call"),
            &[make_type::int_type()],
            &[input],
        )
        .unwrap();
    assert_eq!(get::text(&result), Ok("p4<42>"));
    assert_eq!(recorded.len(), 1);

    let sum_values = make::list(
        &make_type::list_type(make_type::nat_type()),
        vec![make::nat(BigInt::from(2), span("two"))],
        span("values"),
    );
    let sum = interface
        .call_builtin(&mut |_| {}, &id("sum_nat", "sum-call"), &[], &[sum_values])
        .unwrap();
    assert!(matches!(
        get::num(&sum).unwrap(),
        p4spec_rust::lang::xl::num::T::Nat(value) if value == &BigInt::from(2)
    ));
}

#[test]
fn p4_print_checks_arity_before_calling_the_unparser() {
    let mut interface = P4Interface::new(|_| panic!("unparser must not run"));
    let error = interface
        .call_builtin(
            &mut |_| {},
            &id("print_", "bad-print"),
            &[],
            &[make::bool(true, span("value"))],
        )
        .unwrap_err();
    assert_eq!(error.span, span("bad-print"));
    assert!(error.message.contains("arity mismatch"));
}

#[test]
fn p4_print_builds_the_alter_hint_unparser_from_an_sl_spec() {
    let keyword = Spanned::new(Atom::Keyword("WRAPPED".to_owned()), span("keyword"));
    let mixop = Mixfix::Seq(vec![Mixfix::Atom(keyword), Mixfix::Arg(())]);
    let text_hint =
        |text: &str| Spanned::new(el::ExpKind::TextE(text.to_owned()), span("hint-text"));
    let hole = Spanned::new(el::ExpKind::HoleE(el::Hole::Next), span("hint-hole"));
    let hint_exp = Spanned::new(
        el::ExpKind::FuseE(
            Box::new(text_hint("<")),
            Box::new(Spanned::new(
                el::ExpKind::FuseE(Box::new(hole), Box::new(text_hint(">"))),
                span("hint-fuse-right"),
            )),
        ),
        span("hint-fuse"),
    );
    let literal_id = id("literal", "literal");
    let type_case = (
        Spanned::new(
            Mixfix::fill(&mixop, [make_type::text_type()]).unwrap(),
            span("notation"),
        ),
        Spanned::new((literal_id.clone(), Vec::new()), span("origin")),
        vec![el::Hint {
            hintid: id("print", "print-hint"),
            hintexp: hint_exp,
        }],
    );
    let spec = vec![Spanned::new(
        sl::DefKind::TypD(
            literal_id.clone(),
            Vec::new(),
            Spanned::new(il::DefTypKind::VariantT(vec![type_case]), span("variant")),
            Vec::new(),
        ),
        span("type-definition"),
    )];
    let literal_type = make_type::var_type(literal_id, Vec::new());
    let value_case =
        Mixfix::fill(&mixop, [make::text("inside".to_owned(), span("inside"))]).unwrap();
    let value = make::case(&literal_type, value_case, span("value"));
    let mut interface = P4Interface::from_sl_spec(&spec);

    let printed = interface
        .call_builtin(
            &mut |_| {},
            &id("print_", "print-call"),
            std::slice::from_ref(&literal_type),
            &[value],
        )
        .unwrap();
    assert_eq!(get::text(&printed), Ok("<inside>"));
}

#[test]
fn null_interface_fails_without_claiming_side_effects() {
    let mut interface = NullInterface;
    let error = invoke_through_trait(&mut interface, "anything", &[], &[]).unwrap_err();
    assert_eq!(error.span, span("trait-call"));
    assert!(error.message.contains("interface is not configured"));
    assert!(!interface.side_effected(interface.checkpoint(), interface.checkpoint()));
}
