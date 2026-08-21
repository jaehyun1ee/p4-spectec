use num_bigint::BigInt;
use p4spec_rust::{
    domain::{external_data::ExternalData, source::Region},
    interface::{ExternError, SpecCall},
    lang::il::ast::Typ,
    runtime::value::{ValueRef, make},
    sim::{
        core::{
            PacketIn, PacketOut, bits_to_hex, bits_to_signed, bits_to_unsigned, hex_to_bits,
            signed_to_bits, unsigned_to_bits,
        },
        spec::Spec,
    },
};

struct WrongShapeSpec;

impl SpecCall for WrongShapeSpec {
    fn eval_func(
        &mut self,
        _name: &str,
        _type_args: &[Typ],
        _values: &[ValueRef],
    ) -> Result<ValueRef, ExternError> {
        Ok(make::text("not-bits".to_owned(), Region::none()))
    }

    fn eval_rel(
        &mut self,
        _name: &str,
        _values: &[ValueRef],
    ) -> Result<Vec<ValueRef>, ExternError> {
        Ok(Vec::new())
    }
}

#[test]
fn hex_and_bits_match_ocaml_including_odd_nibbles() {
    let bits = hex_to_bits("aF").unwrap();
    assert_eq!(bits_to_hex(&bits), "AF");
    assert_eq!(bits_to_hex(&bits[..5]), "A8");
    assert!(hex_to_bits("G0").is_err());
}

#[test]
fn signed_and_unsigned_bit_conversions_are_width_limited() {
    let bits = vec![true, true, true, true, false, false, false, false];
    assert_eq!(bits_to_unsigned(&bits), BigInt::from(240));
    assert_eq!(bits_to_signed(&bits).unwrap(), BigInt::from(-16));
    assert_eq!(
        unsigned_to_bits(&BigInt::from(18), 8),
        hex_to_bits("12").unwrap()
    );
    assert_eq!(signed_to_bits(&BigInt::from(-16), 8), bits);
}

#[test]
fn packet_cursor_and_payload_are_explicit() {
    let mut packet = PacketIn::new("12AB").unwrap();
    assert_eq!(
        packet.take(8).unwrap(),
        vec![false, false, false, true, false, false, true, false]
    );
    assert_eq!(packet.payload_hex(), "AB");
    assert!(packet.take(9).is_err());
    assert_eq!(packet.payload_hex(), "AB");
}

#[test]
fn packet_output_precedes_the_unparsed_payload() {
    let mut input = PacketIn::new("12AB").unwrap();
    input.advance(8).unwrap();
    let mut output = PacketOut::new();
    output.emit(&hex_to_bits("CD").unwrap());

    assert_eq!(output.packet_hex(&input), "CDAB");
}

#[test]
fn packet_state_has_a_stable_external_representation() {
    let mut packet = PacketIn::new("12AB").unwrap();
    packet.advance(4).unwrap();

    let external = packet.to_external();

    assert!(matches!(external, ExternalData::Assoc(_)));
    assert_eq!(PacketIn::from_external(&external).unwrap(), packet);
}

#[test]
fn spec_wrappers_reject_wrong_value_kinds_and_relation_arities() {
    let mut calls = WrongShapeSpec;
    let mut spec = Spec::new(&mut calls);
    let value = make::text("value".to_owned(), Region::none());

    assert!(spec.write_bits_from_value(&value).is_err());
    let error = spec.ebpf_parse(&value, &value).unwrap_err();
    assert!(
        error
            .to_string()
            .contains("EBPF_parse returned 0 values; expected 3")
    );
}
