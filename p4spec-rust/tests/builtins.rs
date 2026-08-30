use std::rc::Rc;

use num_bigint::BigInt;
use p4spec_rust::{
    interface::builtin::{BuiltinErrorKind, call::Builtins},
    lang::{common::source::Span, traits::print::Print},
    phrase,
    runtime::{
        types::typ,
        value::{ValueRef, get, make},
    },
};

fn id(name: &str) -> p4spec_rust::lang::il::ast::Id {
    phrase!(node: name.to_owned(), span: Span::default())
}

fn invoke(
    builtins: &mut Builtins,
    name: &str,
    values: &[ValueRef],
) -> Result<(ValueRef, Vec<ValueRef>), p4spec_rust::interface::builtin::BuiltinError> {
    invoke_with_types(builtins, name, &[], values)
}

fn invoke_with_types(
    builtins: &mut Builtins,
    name: &str,
    targs: &[p4spec_rust::lang::il::ast::Typ],
    values: &[ValueRef],
) -> Result<(ValueRef, Vec<ValueRef>), p4spec_rust::interface::builtin::BuiltinError> {
    let mut added = Vec::new();
    let value = builtins.invoke(&mut |value| added.push(value), &id(name), targs, values)?;
    Ok((value, added))
}

#[test]
fn numeric_builtin_returns_and_reports_the_same_value() {
    let list_typ = typ::list(typ::int());
    let input = make::list(
        &list_typ,
        vec![
            make::int(BigInt::from(2), Span::default()),
            make::int(BigInt::from(5), Span::default()),
        ],
        Span::default(),
    );

    let (result, added) = invoke(&mut Builtins::new(), "sum_int", &[input]).unwrap();

    assert_eq!(get::num(&result).unwrap().to_string(), "+7");
    assert_eq!(added.len(), 1);
    assert!(Rc::ptr_eq(&result, &added[0]));
}

#[test]
fn fresh_type_ids_are_instance_owned_and_checkpointed() {
    let mut builtins_a = Builtins::new();
    let mut builtins_b = Builtins::new();
    let before = builtins_a.checkpoint();

    let (value_a, _) = invoke(&mut builtins_a, "fresh_typeId", &[]).unwrap();
    let (value_b, _) = invoke(&mut builtins_b, "fresh_typeId", &[]).unwrap();

    assert_eq!(get::text(&value_a), Ok("FRESH__0"));
    assert_eq!(get::text(&value_b), Ok("FRESH__0"));
    assert!(Builtins::side_effected(before, builtins_a.checkpoint()));
    builtins_a.init();
    assert_eq!(builtins_a.checkpoint(), before);
}

#[test]
fn missing_builtin_and_wrong_arity_are_typed_failures() {
    let missing = invoke(&mut Builtins::new(), "missing", &[]).unwrap_err();
    assert!(matches!(
        missing.kind,
        BuiltinErrorKind::MissingImplementation(ref name) if name == "missing"
    ));

    let arity = invoke(&mut Builtins::new(), "sum_int", &[]).unwrap_err();
    assert!(matches!(
        arity.kind,
        BuiltinErrorKind::ArityMismatch {
            expected: 1,
            actual: 0
        }
    ));
}

#[test]
fn list_and_text_builtins_preserve_ocaml_results() {
    let list_typ = typ::list(typ::text());
    let repeated = make::list(
        &list_typ,
        vec![
            make::text("a".to_owned(), Span::default()),
            make::text("a".to_owned(), Span::default()),
        ],
        Span::default(),
    );
    let (distinct, _) = invoke_with_types(
        &mut Builtins::new(),
        "distinct_",
        &[typ::text()],
        &[repeated],
    )
    .unwrap();
    assert_eq!(get::bool(&distinct), Ok(false));

    let text = make::text("  a\n b\t".to_owned(), Span::default());
    let (stripped, _) = invoke(&mut Builtins::new(), "strip_all_whitespace", &[text]).unwrap();
    assert_eq!(get::text(&stripped), Ok("ab"));
}
