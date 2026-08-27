use std::cmp::Ordering;

use num_bigint::BigInt;
use p4spec_rust::{
    lang::common::source::{Position, Span, Spanned},
    lang::traits::print::Print,
    lang::xl::{
        num::{self, BinOp, CmpOp, Natural, Number, NumericError, Typ, UnOp},
        utf8, var,
    },
};

fn natural(value: u64) -> Number {
    Number::Nat(value.into())
}

#[test]
fn utf8_round_trips_valid_codepoints() {
    let codepoints = [0x24, 0xa2, 0x20ac, 0x10348];
    let bytes = utf8::encode(&codepoints).unwrap();

    assert_eq!(
        bytes,
        vec![0x24, 0xc2, 0xa2, 0xe2, 0x82, 0xac, 0xf0, 0x90, 0x8d, 0x88]
    );
    assert_eq!(utf8::decode(&bytes).unwrap(), codepoints);
}

#[test]
fn utf8_encoder_accepts_surrogates_but_decoder_rejects_them() {
    let surrogate = utf8::encode(&[0xd800]).unwrap();

    assert_eq!(surrogate, vec![0xed, 0xa0, 0x80]);
    assert!(utf8::decode(&surrogate).is_err());
}

#[test]
fn utf8_rejects_invalid_codepoints_and_byte_sequences() {
    assert!(utf8::encode(&[-1]).is_err());
    assert!(utf8::encode(&[0x110000]).is_err());

    for bytes in [
        &[0xc0, 0x80][..],
        &[0xed, 0xa0, 0x80][..],
        &[0xf4, 0x90, 0x80, 0x80][..],
        &[0xe2, 0x28, 0xa1][..],
        &[0xf0, 0x90, 0x80][..],
    ] {
        assert!(utf8::decode(bytes).is_err(), "accepted {bytes:02x?}");
    }
}

#[test]
fn strip_var_suffix_preserves_source_and_all_underscore_suffixes() {
    let source = Span::new(
        Position::new("suffix-source", 0, 0),
        Position::new("suffix-source", 0, 0),
    );
    let suffixed = Spanned::new("value_suffix".to_owned(), source.clone());
    let apostrophe = Spanned::new("value'".to_owned(), Span::default());
    let all_underscores = Spanned::new("value___".to_owned(), Span::default());

    let stripped = var::strip_var_suffix(&suffixed);
    assert_eq!(stripped.node, "value");
    assert_eq!(stripped.span, source);
    assert_eq!(var::strip_var_suffix(&apostrophe).node, "value");
    assert_eq!(var::strip_var_suffix(&all_underscores).node, "value___");
}

#[test]
fn numbers_preserve_ocaml_variant_order_and_subtyping() {
    let large_nat = natural(100);
    let small_int = Number::Int(BigInt::from(-100));

    assert_eq!(num::compare(&large_nat, &small_int), Ordering::Less);
    assert_eq!(num::compare_typ(Typ::Nat, Typ::Int), Ordering::Less);
    assert!(num::sub(Typ::Nat, Typ::Int));
    assert!(!num::sub(Typ::Int, Typ::Nat));
}

#[test]
fn numeric_operations_preserve_kinds_and_signed_rendering() {
    let two = natural(2);
    let three = natural(3);
    let negative_three = Number::Int(BigInt::from(-3));

    assert_eq!(num::bin(BinOp::Add, &two, &three), Ok(natural(5)));
    assert_eq!(
        num::bin(BinOp::Sub, &two, &three),
        Ok(Number::Int((-1).into()))
    );
    assert_eq!(num::un(UnOp::Minus, &two), Number::Int((-2).into()));
    assert_eq!(num::cmp(CmpOp::Lt, &two, &three), Ok(true));
    assert_eq!(Print::to_string(&Number::Int(3.into())), "+3");
    assert_eq!(Print::to_string(&negative_three), "-3");
}

#[test]
fn natural_numbers_reject_negative_payloads() {
    assert_eq!(
        Natural::try_from(BigInt::from(-1)),
        Err(NumericError::NegativeNatural(BigInt::from(-1)))
    );
}

#[test]
fn binary_operations_report_zero_divisors() {
    let operands = [
        (natural(5), natural(0)),
        (Number::Int(5.into()), Number::Int(0.into())),
    ];

    for (number_l, number_r) in operands {
        for operation in [BinOp::Div, BinOp::Mod] {
            assert_eq!(
                num::bin(operation, &number_l, &number_r),
                Err(NumericError::ZeroDivisor(operation))
            );
        }
    }
}

#[test]
fn numeric_operations_report_mismatched_kinds() {
    let natural = natural(1);
    let integer = Number::Int(1.into());
    let error = NumericError::MismatchedKinds {
        typ_l: Typ::Nat,
        typ_r: Typ::Int,
    };

    assert_eq!(num::bin(BinOp::Add, &natural, &integer), Err(error.clone()));
    assert_eq!(num::cmp(CmpOp::Lt, &natural, &integer), Err(error));
}

#[test]
fn unsupported_binary_operations_return_errors() {
    assert_eq!(
        num::bin(BinOp::Pow, &natural(2), &natural(3)),
        Err(NumericError::UnsupportedBinaryOperation(BinOp::Pow))
    );
}
