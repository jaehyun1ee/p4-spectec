use p4spec_rust::{
    lang::{
        common::source::Span,
        data::value::{
            ValueArena, ValueError, get, make,
            serde::{decode, encode},
        },
    },
    runner::ExternError,
    sim_plugin::{
        core::object::PacketIn,
        psa::{object::Register, pipe::ObjectState},
    },
};
use serde_json::json;

#[test]
fn test_derived_register_object_preserves_payload_and_annotations() {
    let mut arena = ValueArena::new();
    let span = Span::default();
    let value_typ = make::text(&mut arena, "register type".to_owned(), span.clone()).unwrap();
    let value = make::int(&mut arena, (-123).into(), span.clone()).unwrap();
    let object = ObjectState::Register(Register {
        value_typ,
        values: vec![value],
    });
    let mut json = encode(&arena, &object).unwrap();
    assert_eq!(
        json["Register"]["value_typ"]["node"],
        json!({"Text": "register type"})
    );
    assert_eq!(
        json["Register"]["values"][0]["node"],
        json!({"Num": {"Int": serde_json::to_value(num_bigint::BigInt::from(-123)).unwrap()}})
    );
    let mut arena_decoded = ValueArena::new();
    make::bool(&mut arena_decoded, false, span.clone()).unwrap();
    let object: ObjectState = decode(&mut arena_decoded, &json).unwrap();
    let ObjectState::Register(reg) = &object else {
        panic!("expected register");
    };
    assert_eq!(
        get::text(&arena_decoded, &reg.value_typ).unwrap(),
        "register type"
    );
    assert_eq!(
        get::num(&arena_decoded, &reg.values[0]),
        get::num(&arena, &value)
    );
    assert_eq!(arena_decoded.typ(&reg.values[0]), arena.typ(&value));
    assert_eq!(arena_decoded.span(&reg.values[0]), &span);
    let value_object = object.to_value(&mut arena_decoded).unwrap();
    let object_decoded = ObjectState::from_value(&mut arena_decoded, &value_object).unwrap();
    assert_eq!(encode(&arena_decoded, &object_decoded).unwrap(), json);
    json["Register"]["extra"] = json!(true);
    assert!(decode::<ObjectState>(&mut arena_decoded, &json).is_err());
}

#[test]
fn test_object_packet_bounds_are_checked_when_reading_state() {
    let mut arena = ValueArena::new();
    let mut pkt = PacketIn::init("AB").unwrap();
    pkt.idx = 9;
    let object = ObjectState::PacketIn(pkt);
    let json = encode(&arena, &object).unwrap();
    assert!(decode::<ObjectState>(&mut arena, &json).is_err());
    let value_object = object.to_value(&mut arena).unwrap();
    assert!(ObjectState::from_value(&mut arena, &value_object).is_err());
}

#[test]
fn test_object_codec_rejects_malformed_variant_and_register_records() {
    let mut arena = ValueArena::new();
    for json in [
        json!({"PacketOut": {"bits": []}, "Meter": {}}),
        json!({"Register": {"values": []}}),
        json!({"Unknown": {}}),
    ] {
        assert!(decode::<ObjectState>(&mut arena, &json).is_err());
    }
}

#[test]
fn test_object_restores_nested_register_values_from_its_arena() {
    const DEPTH: usize = 8;
    let mut arena = ValueArena::new();
    let value_typ = make::bool(&mut arena, false, Span::default()).unwrap();
    let mut value = make::bool(&mut arena, true, Span::default()).unwrap();
    for _ in 0..DEPTH {
        value = make::opt(
            &mut arena,
            p4spec_rust::lang::data::typ::TypKind::Bool.into(),
            Some(value),
            Span::default(),
        )
        .unwrap();
    }
    let object = ObjectState::Register(Register {
        value_typ,
        values: vec![value],
    });
    let value_object = object.to_value(&mut arena).unwrap();
    let object = ObjectState::from_value(&mut arena, &value_object).unwrap();
    let ObjectState::Register(reg) = object else {
        panic!("expected register");
    };
    let mut value = reg.values[0];
    for _ in 0..DEPTH {
        value = get::opt(&arena, &value).unwrap().unwrap();
    }
    assert!(get::bool(&arena, &value).unwrap());
    assert!(matches!(
        ObjectState::from_value(&mut arena, &value_typ),
        Err(ExternError::Value(ValueError::UnexpectedKind { .. }))
    ));
    stacker::grow(32 * 1024 * 1024, || drop(arena));
}
