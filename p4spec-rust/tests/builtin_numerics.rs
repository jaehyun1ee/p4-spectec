use num_bigint::BigInt;
use p4spec_rust::{
    domain::source::Region,
    interface::builtin::numerics,
    lang::xl::num,
    runtime::{
        r#type::typ::make as make_type,
        value::{ValueRef, get, make},
    },
};

fn span(file: &str) -> Region {
    Region::for_file(file)
}

fn integer(value: i64) -> ValueRef {
    make::int(BigInt::from(value), span("integer"))
}

fn bigint(value: BigInt) -> ValueRef {
    make::int(value, span("bigint"))
}

fn int_value(value: &ValueRef) -> BigInt {
    match get::num(value).expect("integer") {
        num::T::Int(value) => value.clone(),
        num::T::Nat(_) => panic!("expected integer"),
    }
}

fn bits(values: &[bool]) -> ValueRef {
    make::list(
        &make_type::var_type(
            p4spec_rust::domain::source::Spanned::new("bit".to_owned(), Region::none()),
            Vec::new(),
        ),
        values
            .iter()
            .map(|value| make::bool(*value, Region::none()))
            .collect(),
        span("bits"),
    )
}

fn bit_values(value: &ValueRef) -> Vec<bool> {
    get::list(value)
        .expect("bits")
        .iter()
        .map(|value| get::bool(value).expect("bit"))
        .collect()
}

#[test]
fn shifts_and_power_follow_ocaml_bigint_semantics() {
    let callsite = span("numeric-call");
    let mut add = |_| {};
    let shifted = numerics::shl(&mut add, &callsite, &[], &[integer(3), integer(4)]).unwrap();
    assert_eq!(int_value(&shifted), 48.into());
    let shifted = numerics::shr(&mut add, &callsite, &[], &[integer(17), integer(2)]).unwrap();
    assert_eq!(int_value(&shifted), 4.into());
    let unchanged = numerics::shl(&mut add, &callsite, &[], &[integer(7), integer(-1)]).unwrap();
    assert_eq!(int_value(&unchanged), 7.into());
    let arithmetic = numerics::shr_arith(
        &mut add,
        &callsite,
        &[],
        &[integer(8), integer(2), integer(16)],
    )
    .unwrap();
    assert_eq!(int_value(&arithmetic), 26.into());
    let power = numerics::pow2(&mut add, &callsite, &[], &[integer(5)]).unwrap();
    assert_eq!(int_value(&power), 32.into());

    let error = numerics::shl(&mut add, &callsite, &[], &[integer(1), integer(2049)]).unwrap_err();
    assert_eq!(error.span, callsite);
    assert!(error.message.contains("shift amount too large"));
}

#[test]
fn bitstring_integer_conversions_wrap_at_the_requested_width() {
    let callsite = span("numeric-call");
    let mut add = |_| {};
    let signed =
        numerics::bitstr_to_int(&mut add, &callsite, &[], &[integer(8), integer(255)]).unwrap();
    assert_eq!(int_value(&signed), (-1).into());
    let bitstring =
        numerics::int_to_bitstr(&mut add, &callsite, &[], &[integer(8), integer(-1)]).unwrap();
    assert_eq!(int_value(&bitstring), 255.into());

    let huge_width = bigint(BigInt::from(2049));
    let error =
        numerics::bitstr_to_int(&mut add, &callsite, &[], &[huge_width, integer(0)]).unwrap_err();
    assert!(error.message.contains("bitstr width too large"));
}

#[test]
fn bool_array_conversions_handle_unsigned_signed_and_empty_inputs() {
    let callsite = span("numeric-call");
    let mut add = |_| {};
    let unsigned =
        numerics::bits_to_int_unsigned(&mut add, &callsite, &[], &[bits(&[true, false, true])])
            .unwrap();
    assert_eq!(int_value(&unsigned), 5.into());
    let signed =
        numerics::bits_to_int_signed(&mut add, &callsite, &[], &[bits(&[true, false, true])])
            .unwrap();
    assert_eq!(int_value(&signed), (-3).into());

    let unsigned_bits =
        numerics::int_to_bits_unsigned(&mut add, &callsite, &[], &[integer(4), integer(5)])
            .unwrap();
    assert_eq!(bit_values(&unsigned_bits), vec![false, true, false, true]);
    let signed_bits =
        numerics::int_to_bits_signed(&mut add, &callsite, &[], &[integer(4), integer(-3)]).unwrap();
    assert_eq!(bit_values(&signed_bits), vec![true, true, false, true]);

    let error = numerics::bits_to_int_signed(&mut add, &callsite, &[], &[bits(&[])]).unwrap_err();
    assert_eq!(error.span, Region::none());
    assert!(error.message.contains("empty bit array"));
}

#[test]
fn bitwise_and_slice_operations_match_bigint_operations() {
    let callsite = span("numeric-call");
    let mut add = |_| {};
    let negated = numerics::bneg(&mut add, &callsite, &[], &[integer(5)]).unwrap();
    assert_eq!(int_value(&negated), (-6).into());
    let anded = numerics::band(&mut add, &callsite, &[], &[integer(6), integer(3)]).unwrap();
    assert_eq!(int_value(&anded), 2.into());
    let xored = numerics::bxor(&mut add, &callsite, &[], &[integer(6), integer(3)]).unwrap();
    assert_eq!(int_value(&xored), 5.into());
    let ored = numerics::bor(&mut add, &callsite, &[], &[integer(4), integer(3)]).unwrap();
    assert_eq!(int_value(&ored), 7.into());

    let slice = numerics::bitacc(
        &mut add,
        &callsite,
        &[],
        &[integer(54), integer(4), integer(2)],
    )
    .unwrap();
    assert_eq!(int_value(&slice), 5.into());
    let replaced = numerics::bitacc_replace(
        &mut add,
        &callsite,
        &[],
        &[integer(0), integer(4), integer(2), integer(3)],
    )
    .unwrap();
    assert_eq!(int_value(&replaced), 12.into());
}
