use std::rc::Rc;

use num_bigint::BigInt;
use p4spec_rust::{
    domain::source::Region,
    interface::builtin::{
        BuiltinError, extract,
        ints::{max_int, min_int, sum_int},
        nats::{max_nat, min_nat, sum_nat},
        texts::{
            int_to_text, split_text, strip_all_whitespace, strip_prefix, strip_suffix, text_to_int,
        },
    },
    lang::xl::num,
    runtime::{
        r#type::typ::make as make_type,
        value::{ValueRef, get, make},
    },
};

fn span(file: &str) -> Region {
    Region::for_file(file)
}

fn list(values: Vec<ValueRef>) -> ValueRef {
    make::list(
        &make_type::list_type(make_type::int_type()),
        values,
        span("list"),
    )
}

fn bigint(value: &ValueRef) -> &BigInt {
    match get::num(value).expect("numeric value") {
        num::T::Nat(value) | num::T::Int(value) => value,
    }
}

#[test]
fn extractors_report_arity_errors_at_the_builtin_callsite() {
    let callsite = span("callsite");
    let values = vec![make::bool(true, span("value"))];

    assert_eq!(
        extract::zero(&callsite, &values),
        Err(BuiltinError::new(callsite.clone(), "arity mismatch"))
    );
    assert!(extract::one(&callsite, &values).is_ok());
    assert!(extract::two(&callsite, &values).is_err());
    assert!(extract::three(&callsite, &values).is_err());
    assert!(extract::four(&callsite, &values).is_err());
}

#[test]
fn nat_builtins_fold_lists_and_reject_empty_extrema() {
    let callsite = span("nat-call");
    let values = list(vec![
        make::nat(BigInt::from(7), span("seven")),
        make::int(BigInt::from(2), span("two")),
        make::nat(BigInt::from(5), span("five")),
    ]);
    let inputs = vec![values];
    let mut recorded = Vec::new();
    let (sum, max, min) = {
        let mut add = |value| recorded.push(value);
        let sum = sum_nat(&mut add, &callsite, &[], &inputs).expect("sum nat");
        let max = max_nat(&mut add, &callsite, &[], &inputs).expect("max nat");
        let min = min_nat(&mut add, &callsite, &[], &inputs).expect("min nat");
        let empty = vec![list(Vec::new())];
        assert_eq!(
            max_nat(&mut add, &callsite, &[], &empty),
            Err(BuiltinError::new(callsite.clone(), "max of empty list")),
        );
        assert_eq!(
            min_nat(&mut add, &callsite, &[], &empty),
            Err(BuiltinError::new(callsite, "min of empty list")),
        );
        (sum, max, min)
    };

    assert_eq!(bigint(&sum), &BigInt::from(14));
    assert_eq!(bigint(&max), &BigInt::from(7));
    assert_eq!(bigint(&min), &BigInt::from(2));
    assert!(Rc::ptr_eq(&recorded[0], &sum));
    assert!(Rc::ptr_eq(&recorded[1], &max));
    assert!(Rc::ptr_eq(&recorded[2], &min));
}

#[test]
fn int_builtins_use_zero_for_empty_extrema() {
    let callsite = span("int-call");
    let empty = vec![list(Vec::new())];
    let values = vec![list(vec![
        make::int(BigInt::from(-2), span("minus-two")),
        make::nat(BigInt::from(5), span("five")),
    ])];
    let mut add = |_| {};

    assert_eq!(
        bigint(&sum_int(&mut add, &callsite, &[], &values).unwrap()),
        &BigInt::from(3)
    );
    assert_eq!(
        bigint(&max_int(&mut add, &callsite, &[], &empty).unwrap()),
        &BigInt::from(0)
    );
    assert_eq!(
        bigint(&min_int(&mut add, &callsite, &[], &empty).unwrap()),
        &BigInt::from(0)
    );
}

#[test]
fn text_builtins_preserve_ocaml_string_operations() {
    let callsite = span("text-call");
    let mut recorded = Vec::new();
    let mut add = |value| recorded.push(value);

    let integer = text_to_int(
        &mut add,
        &callsite,
        &[],
        &[make::text("-42".to_owned(), span("number"))],
    )
    .unwrap();
    assert_eq!(bigint(&integer), &BigInt::from(-42));
    let hexadecimal = text_to_int(
        &mut add,
        &callsite,
        &[],
        &[make::text("0xFF".to_owned(), span("hexadecimal"))],
    )
    .unwrap();
    assert_eq!(bigint(&hexadecimal), &BigInt::from(255));
    let binary = text_to_int(
        &mut add,
        &callsite,
        &[],
        &[make::text("-0b1010".to_owned(), span("binary"))],
    )
    .unwrap();
    assert_eq!(bigint(&binary), &BigInt::from(-10));
    let text = int_to_text(&mut add, &callsite, &[], &[integer]).unwrap();
    assert_eq!(get::text(&text).unwrap(), "-42");

    let split = split_text(
        &mut add,
        &callsite,
        &[],
        &[
            make::text("a,b,".to_owned(), span("input")),
            make::text(",".to_owned(), span("separator")),
        ],
    )
    .unwrap();
    let parts = get::list(&split).unwrap();
    assert_eq!(
        parts
            .iter()
            .map(|value| get::text(value).unwrap())
            .collect::<Vec<_>>(),
        vec!["a", "b", ""]
    );

    let prefix = strip_prefix(
        &mut add,
        &callsite,
        &[],
        &[
            make::text("prefix-body".to_owned(), span("input")),
            make::text("prefix-".to_owned(), span("prefix")),
        ],
    )
    .unwrap();
    assert_eq!(get::text(&prefix).unwrap(), "body");
    let suffix = strip_suffix(
        &mut add,
        &callsite,
        &[],
        &[
            make::text("body-suffix".to_owned(), span("input")),
            make::text("-suffix".to_owned(), span("suffix")),
        ],
    )
    .unwrap();
    assert_eq!(get::text(&suffix).unwrap(), "body");
    let compact = strip_all_whitespace(
        &mut add,
        &callsite,
        &[],
        &[make::text(" a\tb c ".to_owned(), span("input"))],
    )
    .unwrap();
    assert_eq!(get::text(&compact).unwrap(), "a\tbc");

    assert_eq!(recorded.len(), 8);
}
