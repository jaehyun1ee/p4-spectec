use std::cmp::Ordering;

use num_bigint::BigInt;
use p4spec_rust::{
    domain::source::{Region, Spanned},
    lang::xl::{
        num::{self, BinOp, CmpOp, T, Typ, UnOp},
        utf8, var,
    },
};

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
    let source = Region::for_file("suffix-source");
    let suffixed = Spanned::new("value_suffix".to_owned(), source.clone());
    let apostrophe = Spanned::new("value'".to_owned(), Region::none());
    let all_underscores = Spanned::new("value___".to_owned(), Region::none());

    let stripped = var::strip_var_suffix(&suffixed);
    assert_eq!(stripped.node, "value");
    assert_eq!(stripped.span, source);
    assert_eq!(var::strip_var_suffix(&apostrophe).node, "value");
    assert_eq!(var::strip_var_suffix(&all_underscores).node, "value___");
}

#[test]
fn numbers_preserve_ocaml_variant_order_and_subtyping() {
    let large_nat = T::Nat(BigInt::from(100));
    let small_int = T::Int(BigInt::from(-100));

    assert_eq!(num::compare(&large_nat, &small_int), Ordering::Less);
    assert_eq!(num::compare_typ(Typ::NatT, Typ::IntT), Ordering::Less);
    assert!(num::sub(Typ::NatT, Typ::IntT));
    assert!(!num::sub(Typ::IntT, Typ::NatT));
}

#[test]
fn numeric_operations_preserve_kinds_and_signed_rendering() {
    let two = T::Nat(BigInt::from(2));
    let three = T::Nat(BigInt::from(3));
    let negative_three = T::Int(BigInt::from(-3));

    assert_eq!(num::bin(BinOp::AddOp, &two, &three), T::Nat(5.into()));
    assert_eq!(num::bin(BinOp::SubOp, &two, &three), T::Int((-1).into()));
    assert_eq!(num::un(UnOp::MinusOp, &two), T::Int((-2).into()));
    assert!(num::cmp(CmpOp::LtOp, &two, &three));
    assert_eq!(num::string_of_num(&T::Int(3.into())), "+3");
    assert_eq!(num::string_of_num(&negative_three), "-3");
}

#[test]
#[should_panic(expected = "invalid numeric binary operation")]
fn numeric_operations_reject_mixed_kinds() {
    num::bin(
        BinOp::AddOp,
        &T::Nat(BigInt::from(1)),
        &T::Int(BigInt::from(1)),
    );
}
