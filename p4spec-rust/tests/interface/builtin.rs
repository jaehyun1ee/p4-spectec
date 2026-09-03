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
) -> Result<(ValueRef, bool), p4spec_rust::interface::builtin::BuiltinError> {
    invoke_with_types(builtins, name, &[], values)
}

fn invoke_with_types(
    builtins: &mut Builtins,
    name: &str,
    targs: &[p4spec_rust::lang::il::ast::Typ],
    values: &[ValueRef],
) -> Result<(ValueRef, bool), p4spec_rust::interface::builtin::BuiltinError> {
    builtins.invoke(&id(name), targs, values)
}

#[test]
fn test_numeric_builtin_returns_value_without_side_effect() {
    let typ_list = typ::list(typ::int());
    let input = make::list(
        &typ_list,
        vec![
            make::int(BigInt::from(2), Span::default()),
            make::int(BigInt::from(5), Span::default()),
        ],
        Span::default(),
    );

    let (result, side_effected) = invoke(&mut Builtins::new(), "sum_int", &[input]).unwrap();

    assert_eq!(get::num(&result).unwrap().to_string(), "+7");
    assert!(!side_effected);
}

#[test]
fn test_fresh_type_ids_are_instance_owned_and_report_side_effects() {
    let mut builtins_a = Builtins::new();
    let mut builtins_b = Builtins::new();

    let (value_a, side_effected_a) = invoke(&mut builtins_a, "fresh_typeId", &[]).unwrap();
    let (value_b, side_effected_b) = invoke(&mut builtins_b, "fresh_typeId", &[]).unwrap();

    assert_eq!(get::text(&value_a), Ok("FRESH__0"));
    assert_eq!(get::text(&value_b), Ok("FRESH__0"));
    assert!(side_effected_a);
    assert!(side_effected_b);

    builtins_a.init();
    let (value_a, side_effected_a) = invoke(&mut builtins_a, "fresh_typeId", &[]).unwrap();
    assert_eq!(get::text(&value_a), Ok("FRESH__0"));
    assert!(side_effected_a);
}

#[test]
fn test_missing_builtin_and_wrong_arity_are_typed_failures() {
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
fn test_list_and_text_builtins_preserve_ocaml_results() {
    let typ_list = typ::list(typ::text());
    let repeated = make::list(
        &typ_list,
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
    assert_eq!(get::text(&stripped), Ok("a\nb\t"));
}

#[test]
fn test_int_to_text_preserves_the_explicit_integer_sign() {
    let integer = make::int(7.into(), Span::default());
    let natural = make::nat(7_u64.into(), Span::default());

    let (integer_text, _) = invoke(&mut Builtins::new(), "int_to_text", &[integer]).unwrap();
    let (natural_text, _) = invoke(&mut Builtins::new(), "int_to_text", &[natural]).unwrap();

    assert_eq!(get::text(&integer_text), Ok("+7"));
    assert_eq!(get::text(&natural_text), Ok("7"));
}

#[test]
fn test_zero_and_negative_bit_widths_match_ocaml() {
    for width in [0, -1] {
        for name in ["bitstr_to_int", "int_to_bitstr"] {
            let values = [
                make::int(width.into(), Span::default()),
                make::int(17.into(), Span::default()),
            ];
            let (result, _) = invoke(&mut Builtins::new(), name, &values).unwrap();
            assert_eq!(get::num(&result).unwrap().to_string(), "+0", "{name}");
        }
    }
}

#[test]
fn test_negative_array_width_is_rejected_like_ocaml_array_init() {
    for name in ["int_to_bits_unsigned", "int_to_bits_signed"] {
        let values = [
            make::int((-1).into(), Span::default()),
            make::int(17.into(), Span::default()),
        ];
        let result = invoke(&mut Builtins::new(), name, &values);

        assert!(result.is_err(), "{name}");
    }
}
