use serde_json::json;

use p4spec_rust::lang::{
    common::{
        notation::{atom::Atom, mixfix::Mixfix},
        source::{Position, Span},
    },
    data::{
        typ,
        value::{Value, ValueArena, get, make, serde as payload},
    },
};

fn span_at(line: i64) -> Span {
    Span::new(
        Position::new("queued.p4", line, 2),
        Position::new("queued.p4", line, 9),
    )
}

#[test]
fn test_native_payload_restores_annotated_context_in_fresh_arena() {
    let mut arena = ValueArena::new();
    let data = json!({
        "at": "opaque location",
        "note": {"it": ["ExternV", null], "note": false},
        "tuple": [true, -9],
        "variant": ["Some", [null, -0.0]],
        "int": u64::MAX,
    });
    let typ_external = typ::make::var(
        p4spec_rust::phrase!(node: "objectState".to_owned(), span: span_at(1)),
        vec![typ::make::bool()],
    );
    let value_external = make::external(
        &mut arena,
        typ_external.node.into(),
        data.clone(),
        span_at(2),
    )
    .unwrap();
    let value_external_relocated = arena.update_span(value_external, span_at(3)).unwrap();
    let value_case = Mixfix::Brack(
        p4spec_rust::phrase!(node: Atom::LParen, span: span_at(4)),
        Box::new(Mixfix::Infix(
            Box::new(Mixfix::Arg(value_external)),
            p4spec_rust::phrase!(node: Atom::Arrow, span: span_at(5)),
            Box::new(Mixfix::Seq(vec![Mixfix::Arg(value_external_relocated)])),
        )),
        p4spec_rust::phrase!(node: Atom::RParen, span: span_at(6)),
    );
    let value_case = make::case(
        &mut arena,
        typ::TypKind::Text.into(),
        value_case,
        span_at(7),
    )
    .unwrap();
    let value_some = make::opt(
        &mut arena,
        typ::make::opt(typ::make::text()).node.into(),
        Some(value_case),
        span_at(8),
    )
    .unwrap();
    let value = make::structure(
        &mut arena,
        typ::TypKind::Text.into(),
        vec![(
            p4spec_rust::phrase!(node: Atom::keyword("context"), span: span_at(9)),
            value_some,
        )],
        span_at(10),
    )
    .unwrap();
    let data_value = payload::encode(&arena, &value).unwrap();
    let mut arena_decoded = ValueArena::new();
    make::bool(&mut arena_decoded, false, Span::default()).unwrap();
    let value_decoded = payload::decode(&mut arena_decoded, &data_value).unwrap();
    assert_eq!(
        payload::encode(&arena_decoded, &value_decoded).unwrap(),
        data_value
    );
    assert_eq!(arena_decoded.span(&value_decoded), &span_at(10));
    let fields = get::structure(&arena_decoded, &value_decoded).unwrap();
    assert_eq!(fields[0].0.span, span_at(9));
    let value_case = get::opt(&arena_decoded, &fields[0].1).unwrap().unwrap();
    let values = get::case(&arena_decoded, &value_case).unwrap().args();
    assert_eq!(arena_decoded.span(values[0]), &span_at(2));
    assert_eq!(arena_decoded.span(values[1]), &span_at(3));
    assert_eq!(arena_decoded.typ(values[0]), arena.typ(&value_external),);
    assert_eq!(get::external(&arena_decoded, values[0]).unwrap(), &data);
    assert_eq!(get::external(&arena_decoded, values[1]).unwrap(), &data);
}

#[test]
fn test_native_payload_rejects_missing_metadata_and_nested_constructor_arity() {
    let mut arena = ValueArena::new();
    let value = make::bool(&mut arena, true, span_at(1)).unwrap();
    let value = make::list(
        &mut arena,
        typ::make::list(typ::make::bool()).node.into(),
        vec![value],
        span_at(2),
    )
    .unwrap();
    let mut json_value = payload::encode(&arena, &value).unwrap();
    let mut arena_decoded = ValueArena::new();
    make::bool(&mut arena_decoded, false, Span::default()).unwrap();
    let mut json_missing = json_value.clone();
    json_missing.as_object_mut().unwrap().remove("span");
    assert!(payload::decode::<Value>(&mut arena_decoded, &json_missing).is_err());
    json_value["node"]["List"][0]["node"] = json!({"Bool": []});
    assert!(payload::decode::<Value>(&mut arena_decoded, &json_value).is_err());
}

#[test]
fn test_native_payload_preserves_wide_source_positions() {
    let mut arena = ValueArena::new();
    let value = make::bool(&mut arena, true, span_at(i64::MAX)).unwrap();
    use serde_state::{DeserializeState, SerializeState};
    let mut bytes = Vec::new();
    value
        .serialize_state(&mut serde_json::Serializer::new(&mut bytes), &arena)
        .unwrap();
    let mut arena_decoded = ValueArena::new();
    let mut deserializer = serde_json::Deserializer::from_slice(&bytes);
    let value_decoded = Value::deserialize_state(&mut arena_decoded, &mut deserializer).unwrap();
    deserializer.end().unwrap();
    assert_eq!(arena_decoded.span(&value_decoded), &span_at(i64::MAX));
}

#[test]
fn test_native_payload_restores_wide_register_values_and_callable_metadata() {
    let mut arena = ValueArena::new();
    let int = num_bigint::BigInt::from(1) << 160_usize;
    let value_nat = make::nat(&mut arena, int.clone().try_into().unwrap(), span_at(1)).unwrap();
    let value_int = make::int(&mut arena, -&int, span_at(2)).unwrap();
    let value_text = make::text(&mut arena, "saved register".to_owned(), span_at(3)).unwrap();
    let value_none = make::opt(
        &mut arena,
        typ::make::opt(typ::make::text()).node.into(),
        None,
        span_at(4),
    )
    .unwrap();
    let id = p4spec_rust::phrase!(node: "restore".to_owned(), span: span_at(5));
    let value_func = make::func(
        &mut arena,
        id.clone(),
        vec![],
        vec![typ::make::text()],
        typ::make::bool(),
        span_at(6),
    )
    .unwrap();
    let values = vec![value_nat, value_int, value_text, value_none, value_func];
    let typ = typ::make::tuple(values.iter().map(|value| {
        p4spec_rust::phrase!(node: arena.typ(value).as_ref().clone(), span: arena.span(value).clone())
    }).collect());
    let value = make::tuple(&mut arena, typ.node.into(), values.clone(), span_at(7)).unwrap();
    let bytes = serde_json::to_vec(&payload::encode(&arena, &value).unwrap()).unwrap();
    let json_value = serde_json::from_slice(&bytes).unwrap();
    let mut arena_decoded = ValueArena::new();
    make::bool(&mut arena_decoded, false, Span::default()).unwrap();
    let value_decoded = payload::decode(&mut arena_decoded, &json_value).unwrap();
    let values_decoded = get::tuple(&arena_decoded, &value_decoded).unwrap();
    assert_eq!(
        get::num(&arena_decoded, &values_decoded[0]),
        get::num(&arena, &value_nat)
    );
    assert_eq!(
        get::num(&arena_decoded, &values_decoded[1]),
        get::num(&arena, &value_int)
    );
    assert_eq!(
        get::text(&arena_decoded, &values_decoded[2]).unwrap(),
        "saved register"
    );
    assert_eq!(get::opt(&arena_decoded, &values_decoded[3]).unwrap(), None);
    assert_eq!(get::func(&arena_decoded, &values_decoded[4]).unwrap(), &id);
    for (value, value_decoded) in values.iter().zip(values_decoded) {
        assert_eq!(arena.typ(value), arena_decoded.typ(value_decoded));
        assert_eq!(arena.span(value), arena_decoded.span(value_decoded));
    }
}

#[test]
fn test_serde_rejects_negative_natural() {
    use p4spec_rust::lang::xl::num::{Natural, Number};
    let json_negative = serde_json::to_value(num_bigint::BigInt::from(-1)).unwrap();
    assert!(serde_json::from_value::<Natural>(json_negative.clone()).is_err());
    let mut arena = ValueArena::new();
    let value = make::nat(&mut arena, Natural::from(1), Span::default()).unwrap();
    let mut json_value = payload::encode(&arena, &value).unwrap();
    json_value["node"]["Num"]["Nat"] = json_negative;
    assert!(payload::decode::<Value>(&mut arena, &json_value).is_err());
    let json_value = payload::encode(&arena, &value).unwrap();
    let value: Value = payload::decode(&mut arena, &json_value).unwrap();
    assert_eq!(
        get::num(&arena, &value).unwrap(),
        &Number::Nat(Natural::from(1))
    );
}
