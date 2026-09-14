use std::rc::Rc;

use serde_json::json;

use p4spec_rust::lang::{
    common::{
        notation::{atom::Atom, mixfix::Mixfix},
        source::{Position, Span},
    },
    data::{
        typ,
        value::{Value, ValueArena, external as payload, get, make},
    },
};

fn span_at(line: i64) -> Span {
    Span::new(
        Position::new("queued.p4", line, 2),
        Position::new("queued.p4", line, 9),
    )
}

#[test]
fn test_independent_restores_contents_and_annotations_after_source_arena_drop() {
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
        data.clone().into(),
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
    let typ_external = arena.typ(&value_external).clone();
    let data_value = payload::encode(&arena, &value).unwrap();
    drop(arena);
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
    assert_eq!(arena_decoded.typ(values[0]), &typ_external);
    assert_eq!(arena_decoded.typ(values[1]), &typ_external);
    assert_eq!(
        get::external(&arena_decoded, values[0]).unwrap().as_ref(),
        &data
    );
    assert_eq!(
        get::external(&arena_decoded, values[1]).unwrap().as_ref(),
        &data
    );
}

#[test]
fn test_relative_preserves_full_handles_without_interning() {
    use payload::Encoding::ArenaRelative;
    let mut arena = ValueArena::new();
    let value = make::text(&mut arena, "shared".into(), span_at(41)).unwrap();
    let value_retyped = arena.update_typ(value, typ::TypKind::Bool.into()).unwrap();
    let value_relocated = arena.update_span(value, span_at(42)).unwrap();
    let values = vec![value, value_retyped, value_relocated];
    let json_values = payload::encode_with(&arena, ArenaRelative, &values).unwrap();
    let text_arena = format!("{arena:?}");
    for _ in 0..20 {
        let values_restored: Vec<Value> =
            payload::decode_with(&mut arena, ArenaRelative, &json_values).unwrap();
        for (value, value_restored) in values.iter().zip(&values_restored) {
            assert_eq!(value_restored.node, value.node);
            assert_eq!(value_restored.note, value.note);
            assert_eq!(value_restored.span, value.span);
        }
        assert_eq!(values_restored, values);
    }
    assert_eq!(format!("{arena:?}"), text_arena);
}

#[test]
fn test_relative_raw_indices_without_arena_lookup() {
    use p4spec_rust::lang::data::value::{Interned, ValueKind};
    use payload::Encoding::ArenaRelative;
    let mut arena = ValueArena::new();
    let text_arena = format!("{arena:?}");
    for (value_idx, typ_idx, span_idx) in [(0, 17, u32::MAX), (17, u32::MAX, 0), (u32::MAX, 0, 17)]
    {
        let json_value = json!({"node": value_idx, "note": typ_idx, "span": span_idx});
        let value: Value = payload::decode_with(&mut arena, ArenaRelative, &json_value).unwrap();
        assert_eq!(
            payload::encode_with(&arena, ArenaRelative, &value).unwrap(),
            json_value
        );
        let value_kind: Interned<ValueKind> =
            payload::decode_with(&mut arena, ArenaRelative, &json!(value_idx)).unwrap();
        let typ_kind: Interned<typ::TypKind> =
            payload::decode_with(&mut arena, ArenaRelative, &json!(typ_idx)).unwrap();
        let span: Interned<Span> =
            payload::decode_with(&mut arena, ArenaRelative, &json!(span_idx)).unwrap();
        assert_eq!(value.node, value_kind);
        assert_eq!(value.note, typ_kind);
        assert_eq!(value.span, span);
        assert_eq!(
            payload::encode_with(&arena, ArenaRelative, &value_kind).unwrap(),
            json!(value_idx)
        );
        assert_eq!(
            payload::encode_with(&arena, ArenaRelative, &typ_kind).unwrap(),
            json!(typ_idx)
        );
        assert_eq!(
            payload::encode_with(&arena, ArenaRelative, &span).unwrap(),
            json!(span_idx)
        );
    }
    assert_eq!(format!("{arena:?}"), text_arena);
}

#[test]
fn test_relative_state_serde_needs_no_arena() {
    use serde_state::{DeserializeState, SerializeState};

    let json_value = json!({"node": u32::MAX, "note": 17, "span": 0});
    let value =
        Value::deserialize_state(&mut payload::DecodeContext::ArenaRelative, &json_value).unwrap();
    let json_restored = value
        .serialize_state(
            serde_json::value::Serializer,
            &payload::EncodeContext::ArenaRelative,
        )
        .unwrap();
    assert_eq!(json_restored, json_value);
}

#[test]
fn test_plain_primitives_have_same_json_in_both_modes() {
    use payload::Encoding::{ArenaIndependent, ArenaRelative};
    let mut arena = ValueArena::new();
    let data = (true, -9_i64, u64::MAX, "plain".to_owned(), None::<bool>);
    let json_expect = json!([true, -9, u64::MAX, "plain", null]);
    for encoding in [ArenaIndependent, ArenaRelative] {
        let json_data = payload::encode_with(&arena, encoding, &data).unwrap();
        assert_eq!(json_data, json_expect);
        for encoding in [ArenaIndependent, ArenaRelative] {
            let data_restored: (bool, i64, u64, String, Option<bool>) =
                payload::decode_with(&mut arena, encoding, &json_data).unwrap();
            assert_eq!(data_restored, data);
        }
    }
}

#[test]
fn test_independent_native_payload_survives_source_arena_drop() {
    use p4spec_rust::sim_plugin::psa::object::Register;
    use payload::Encoding::ArenaIndependent;
    let native = {
        let mut arena = ValueArena::new();
        let value_typ = make::text(&mut arena, "T".into(), span_at(50)).unwrap();
        let value = make::bool(&mut arena, true, span_at(51)).unwrap();
        Rc::new(
            payload::encode_with(
                &arena,
                ArenaIndependent,
                &Register {
                    value_typ,
                    values: vec![value],
                },
            )
            .unwrap(),
        )
    };
    let mut arena = ValueArena::new();
    make::text(&mut arena, "unrelated".into(), Span::default()).unwrap();
    let value = make::external(
        &mut arena,
        typ::TypKind::Text.into(),
        native.clone(),
        span_at(52),
    )
    .unwrap();
    let register: Register = payload::decode_external(&mut arena, &value).unwrap();
    assert_eq!(get::text(&arena, &register.value_typ).unwrap(), "T");
    assert_eq!(arena.typ(&register.value_typ).as_ref(), &typ::TypKind::Text);
    assert_eq!(arena.span(&register.value_typ), &span_at(50));
    assert!(get::bool(&arena, &register.values[0]).unwrap());
    assert_eq!(arena.typ(&register.values[0]).as_ref(), &typ::TypKind::Bool);
    assert_eq!(arena.span(&register.values[0]), &span_at(51));
    assert_eq!(
        payload::encode(&arena, &register).unwrap(),
        *native.as_ref()
    );
}

#[test]
fn test_independent_export_preserves_stored_relative_extern_json() {
    use p4spec_rust::sim_plugin::psa::object::Register;
    use payload::Encoding::ArenaRelative;
    let mut arena = ValueArena::new();
    let value_typ = make::text(&mut arena, "T".into(), span_at(53)).unwrap();
    let value = make::bool(&mut arena, true, span_at(54)).unwrap();
    let register = Register {
        value_typ,
        values: vec![value],
    };
    let native = Rc::new(payload::encode_with(&arena, ArenaRelative, &register).unwrap());
    let json_native = native.as_ref().clone();
    assert_eq!(serde_json::to_value(&native).unwrap(), json_native);
    let value_external =
        make::external(&mut arena, typ::TypKind::Text.into(), native, span_at(55)).unwrap();
    let json_value = payload::encode(&arena, &value_external).unwrap();
    assert_eq!(json_value["node"]["Extern"], json_native);
    let json_external = get::external(&arena, &value_external).unwrap().clone();
    let register_restored: Register =
        payload::decode_with(&mut arena, ArenaRelative, &json_external).unwrap();
    assert_eq!(register_restored, register);
    let register_outer = Register {
        value_typ,
        values: vec![value_external],
    };
    let json_outer = payload::encode(&arena, &register_outer).unwrap();
    assert_eq!(json_outer["values"][0]["node"]["Extern"], json_native);
    drop(arena);
    let mut arena_decoded = ValueArena::new();
    let value_decoded: Value = payload::decode(&mut arena_decoded, &json_value).unwrap();
    assert_eq!(
        get::external(&arena_decoded, &value_decoded)
            .unwrap()
            .as_ref(),
        &json_native
    );
    assert_eq!(
        payload::encode(&arena_decoded, &value_decoded).unwrap(),
        json_value
    );
}

#[test]
fn test_native_json_differs_by_mode_and_relative_snapshots_preserve_handles() {
    use p4spec_rust::sim_plugin::psa::object::Register;
    use payload::Encoding::{ArenaIndependent, ArenaRelative};
    let mut arena = ValueArena::new();
    let value_typ = make::text(&mut arena, "T".into(), span_at(60)).unwrap();
    let value = make::bool(&mut arena, true, span_at(61)).unwrap();
    let register = Register {
        value_typ,
        values: vec![value],
    };
    let native_relative = Rc::new(payload::encode_with(&arena, ArenaRelative, &register).unwrap());
    let native_independent =
        Rc::new(payload::encode_with(&arena, ArenaIndependent, &register).unwrap());
    assert_ne!(native_relative, native_independent);
    let value_relative = make::external(
        &mut arena,
        typ::TypKind::Text.into(),
        native_relative,
        Span::default(),
    )
    .unwrap();
    let value_independent = make::external(
        &mut arena,
        typ::TypKind::Text.into(),
        native_independent,
        Span::default(),
    )
    .unwrap();
    assert_ne!(
        arena.canon_id(&value_relative),
        arena.canon_id(&value_independent)
    );
    let text_arena = format!("{arena:?}");
    let json_external = get::external(&arena, &value_relative).unwrap().clone();
    let mut register_changed: Register =
        payload::decode_with(&mut arena, ArenaRelative, &json_external).unwrap();
    assert_eq!(register_changed, register);
    assert_eq!(format!("{arena:?}"), text_arena);
    register_changed.values[0] = make::bool(&mut arena, false, span_at(62)).unwrap();
    let native_changed =
        Rc::new(payload::encode_with(&arena, ArenaRelative, &register_changed).unwrap());
    let value_changed = make::external(
        &mut arena,
        typ::TypKind::Text.into(),
        native_changed,
        Span::default(),
    )
    .unwrap();
    assert_ne!(
        arena.canon_id(&value_changed),
        arena.canon_id(&value_relative)
    );
    let json_external = get::external(&arena, &value_relative).unwrap().clone();
    let register_restored: Register =
        payload::decode_with(&mut arena, ArenaRelative, &json_external).unwrap();
    assert_eq!(register_restored, register);
    let native_restored =
        Rc::new(payload::encode_with(&arena, ArenaRelative, &register_restored).unwrap());
    let value_restored = make::external(
        &mut arena,
        typ::TypKind::Text.into(),
        native_restored,
        Span::default(),
    )
    .unwrap();
    assert_eq!(value_restored.node, value_relative.node);
}

#[test]
fn test_relative_native_canonical_equality_retains_nested_value_handles() {
    use p4spec_rust::sim_plugin::psa::object::Register;
    use payload::Encoding::ArenaRelative;
    let mut arena = ValueArena::new();
    let value_typ = make::text(&mut arena, "T".into(), span_at(70)).unwrap();
    let value = make::bool(&mut arena, true, span_at(71)).unwrap();
    let value_retyped = arena.update_typ(value, typ::TypKind::Text.into()).unwrap();
    let value_relocated = arena.update_span(value, span_at(72)).unwrap();
    let mut values_external = Vec::new();
    for value in [value, value_retyped, value_relocated] {
        let value_list = make::list(
            &mut arena,
            typ::make::list(typ::make::bool()).node.into(),
            vec![value],
            span_at(73),
        )
        .unwrap();
        let register = Register {
            value_typ,
            values: vec![value_list],
        };
        let native = Rc::new(payload::encode_with(&arena, ArenaRelative, &register).unwrap());
        let value_external = make::external(
            &mut arena,
            typ::TypKind::Text.into(),
            native,
            Span::default(),
        )
        .unwrap();
        let json_external = get::external(&arena, &value_external).unwrap().clone();
        let register_restored: Register =
            payload::decode_with(&mut arena, ArenaRelative, &json_external).unwrap();
        assert_eq!(register_restored, register);
        assert_eq!(
            get::list(&arena, &register_restored.values[0]).unwrap(),
            &[value]
        );
        values_external.push(value_external);
    }
    for value_external in &values_external[1..] {
        assert_ne!(values_external[0].node, value_external.node);
        assert_ne!(
            get::external(&arena, &values_external[0]).unwrap(),
            get::external(&arena, value_external).unwrap(),
        );
        assert_ne!(
            arena.canon_id(&values_external[0]),
            arena.canon_id(value_external)
        );
    }
}

#[test]
fn test_relative_native_order_uses_json_handles_and_annotations() {
    use p4spec_rust::lang::traits::{cmp::SyntaxCmp, eq::SyntaxEq};
    use payload::Encoding::ArenaRelative;
    let mut arena = ValueArena::new();
    let value_large = make::int(&mut arena, 20.into(), span_at(1)).unwrap();
    let value_small = make::int(&mut arena, 3.into(), span_at(2)).unwrap();
    let value_relocated = arena.update_span(value_small, span_at(3)).unwrap();
    let mut values = Vec::new();
    for value in [value_large, value_small, value_relocated] {
        let native = Rc::new(payload::encode_with(&arena, ArenaRelative, &vec![value]).unwrap());
        values.push(
            make::external(
                &mut arena,
                typ::TypKind::Text.into(),
                native,
                Span::default(),
            )
            .unwrap(),
        );
    }
    assert!(
        arena
            .view(values[0])
            .syntax_cmp(&arena.view(values[1]))
            .is_lt()
    );
    assert!(
        arena
            .view(values[1])
            .syntax_cmp(&arena.view(values[2]))
            .is_lt()
    );
    assert!(!arena.view(values[1]).syntax_eq(&arena.view(values[2])));
}

#[test]
fn test_relative_native_equality_retains_nested_extern_handles() {
    use payload::Encoding::ArenaRelative;
    let mut arena = ValueArena::new();
    let value = make::bool(&mut arena, true, span_at(1)).unwrap();
    let value_relocated = arena.update_span(value, span_at(2)).unwrap();
    let mut values = Vec::new();
    for mut value in [value, value_relocated] {
        for _ in 0..3 {
            let native =
                Rc::new(payload::encode_with(&arena, ArenaRelative, &vec![value]).unwrap());
            value = make::external(
                &mut arena,
                typ::TypKind::Text.into(),
                native,
                Span::default(),
            )
            .unwrap();
        }
        values.push(value);
    }
    assert_ne!(values[0].node, values[1].node);
    assert_ne!(arena.canon_id(&values[0]), arena.canon_id(&values[1]));
    for value in values {
        let json_value = payload::encode(&arena, &value).unwrap();
        assert_eq!(
            json_value["node"]["Extern"],
            *get::external(&arena, &value).unwrap().as_ref()
        );
    }
}

#[test]
fn test_independent_native_equality_retains_value_annotations() {
    use payload::Encoding::ArenaIndependent;
    let mut arena = ValueArena::new();
    let value = make::bool(&mut arena, true, span_at(74)).unwrap();
    let value_retyped = arena.update_typ(value, typ::TypKind::Text.into()).unwrap();
    let value_relocated = arena.update_span(value, span_at(75)).unwrap();
    let native = Rc::new(payload::encode_with(&arena, ArenaIndependent, &vec![value]).unwrap());
    let value_external = make::external(
        &mut arena,
        typ::TypKind::Text.into(),
        native.clone(),
        Span::default(),
    )
    .unwrap();
    for value_other in [value_retyped, value_relocated] {
        assert_eq!(arena.canon_id(&value), arena.canon_id(&value_other));
        let native_other =
            Rc::new(payload::encode_with(&arena, ArenaIndependent, &vec![value_other]).unwrap());
        assert_ne!(native.as_ref(), native_other.as_ref());
        assert_ne!(native, native_other);
        let value_external_other = make::external(
            &mut arena,
            typ::TypKind::Text.into(),
            native_other,
            Span::default(),
        )
        .unwrap();
        assert_ne!(
            arena.canon_id(&value_external),
            arena.canon_id(&value_external_other)
        );
    }
}

#[test]
fn test_independent_extern_equality_and_hash_follow_original_json() {
    use std::{
        collections::hash_map::DefaultHasher,
        hash::{Hash, Hasher},
    };
    let mut arena = ValueArena::new();
    let jsons = [
        json!(0.0),
        json!(-0.0),
        json!(0),
        json!(51.248178375505404_f64),
    ];
    for json_a in &jsons {
        for json_b in &jsons {
            let native_a = Rc::new(json_a.clone());
            let native_b = Rc::new(json_b.clone());
            assert_eq!(native_a == native_b, json_a == json_b);
            if json_a == json_b {
                let mut hasher_a = DefaultHasher::new();
                let mut hasher_b = DefaultHasher::new();
                native_a.hash(&mut hasher_a);
                native_b.hash(&mut hasher_b);
                assert_eq!(hasher_a.finish(), hasher_b.finish());
            }
            let value_a = make::external(
                &mut arena,
                typ::TypKind::Text.into(),
                native_a,
                Span::default(),
            )
            .unwrap();
            let value_b = make::external(
                &mut arena,
                typ::TypKind::Text.into(),
                native_b,
                Span::default(),
            )
            .unwrap();
            assert_eq!(
                arena.canon_id(&value_a) == arena.canon_id(&value_b),
                json_a == json_b
            );
        }
    }
}

#[test]
fn test_native_codec_encodes_borrowed_values_without_deserialization() {
    use payload::Encoding::{ArenaIndependent, ArenaRelative};
    let mut arena = ValueArena::new();
    let value = make::bool(&mut arena, true, span_at(76)).unwrap();
    for encoding in [ArenaIndependent, ArenaRelative] {
        let json_native = payload::encode_with(&arena, encoding, &&value).unwrap();
        let value_restored: Value =
            payload::decode_with(&mut arena, encoding, &json_native).unwrap();
        match encoding {
            ArenaRelative => assert_eq!(value_restored, value),
            ArenaIndependent => assert_eq!(
                payload::encode(&arena, &value_restored).unwrap(),
                payload::encode(&arena, &value).unwrap(),
            ),
        }
    }
}

#[test]
fn test_direct_external_fields_preserve_opaque_json_in_both_modes() {
    use payload::Encoding::{ArenaIndependent, ArenaRelative};
    use serde_derive_state::{DeserializeState, SerializeState};
    use serde_json::Value as Json;

    #[derive(Debug, PartialEq, SerializeState, DeserializeState)]
    #[serde(serialize_state = "State", ser_parameters = "State")]
    #[serde(deserialize_state = "State", de_parameters = "State")]
    struct Native {
        jsons: Vec<Rc<Json>>,
    }

    let mut arena = ValueArena::new();
    let value = make::bool(&mut arena, true, span_at(80)).unwrap();
    let json = Rc::new(payload::encode_with(&arena, ArenaRelative, &value).unwrap());
    let native = Native {
        jsons: vec![json.clone()],
    };
    let json_expect = json!({"jsons": [json.as_ref()]});
    drop(arena);
    for encoding in [ArenaIndependent, ArenaRelative] {
        let mut arena = ValueArena::new();
        let json_native = Rc::new(payload::encode_with(&arena, encoding, &native).unwrap());
        assert_eq!(json_native.as_ref(), &json_expect);
        let native_restored: Native =
            payload::decode_with(&mut arena, encoding, json_native.as_ref()).unwrap();
        assert_eq!(native_restored, native);
        let value = make::external(
            &mut arena,
            typ::TypKind::Text.into(),
            native_restored.jsons[0].clone(),
            span_at(81),
        )
        .unwrap();
        assert_eq!(
            get::external(&arena, &value).unwrap().as_ref(),
            json.as_ref()
        );
        assert!(
            matches!(arena.kind(&value), p4spec_rust::lang::data::value::ValueKind::Extern(json)
            if Rc::ptr_eq(json, &native_restored.jsons[0]))
        );
    }
}

#[test]
fn test_nested_external_json_is_opaque_in_both_modes() {
    use p4spec_rust::lang::data::value::ValueKind;
    use payload::Encoding::{ArenaIndependent, ArenaRelative};
    let json_opaque = json!({
        "$p4spec-arena": {"version": 999, "encoding": "anything", "arena": null},
        "payload": [{
            "node": u32::MAX, "note": -1, "span": "opaque", "kind": "unknown",
            "nested": {"$p4spec-arena": false, "payload": [null, true, -0.0]},
        }],
    });
    for encoding in [ArenaIndependent, ArenaRelative] {
        let mut arena = ValueArena::new();
        let native = ValueKind::Extern(Rc::new(json_opaque.clone()));
        assert_eq!(
            payload::encode_with(&arena, encoding, &native).unwrap(),
            json!({"Extern": json_opaque})
        );
        let native_decoded: ValueKind =
            payload::decode_with(&mut arena, encoding, &json!({"Extern": json_opaque})).unwrap();
        assert_eq!(native_decoded, native);
        let value = make::external(
            &mut arena,
            typ::TypKind::Text.into(),
            json_opaque.clone().into(),
            span_at(82),
        )
        .unwrap();
        let json_value = payload::encode(&arena, &value).unwrap();
        assert_eq!(json_value["node"]["Extern"], json_opaque);
        drop(arena);
        let mut arena_decoded = ValueArena::new();
        let value_decoded: Value = payload::decode(&mut arena_decoded, &json_value).unwrap();
        assert_eq!(
            get::external(&arena_decoded, &value_decoded)
                .unwrap()
                .as_ref(),
            &json_opaque
        );
        assert_eq!(
            payload::encode(&arena_decoded, &value_decoded).unwrap(),
            json_value
        );
    }
}

#[test]
fn test_independent_ast_serde_matches_json_and_restores_annotations() {
    use p4spec_rust::lang::data::value::external::indep;
    let mut arena = ValueArena::new();
    let value_bool = make::bool(&mut arena, true, span_at(90)).unwrap();
    let value_text = make::text(&mut arena, "saved".to_owned(), span_at(91)).unwrap();
    let value = make::tuple(
        &mut arena,
        typ::TypKind::Text.into(),
        vec![value_bool, value_text],
        span_at(92),
    )
    .unwrap();
    let json_span = |line| {
        json!({
            "left": {"file": "queued.p4", "line": line, "column": 2},
            "right": {"file": "queued.p4", "line": line, "column": 9},
        })
    };
    let json_expect = json!({
        "node": {"Tuple": [
            {"node": {"Bool": true}, "note": "Bool", "span": json_span(90)},
            {"node": {"Text": "saved"}, "note": "Text", "span": json_span(91)},
        ]},
        "note": "Text",
        "span": json_span(92),
    });
    let value_tree: indep::Value = indep::from_arena(&arena, &value);
    assert_eq!(serde_json::to_value(&value_tree).unwrap(), json_expect);
    assert_eq!(payload::encode(&arena, &value).unwrap(), json_expect);
    drop(arena);
    let value_tree: indep::Value = serde_json::from_value(json_expect.clone()).unwrap();
    let mut arena_decoded = ValueArena::new();
    make::text(&mut arena_decoded, "unrelated".to_owned(), Span::default()).unwrap();
    let value_decoded = indep::into_arena(&mut arena_decoded, value_tree).unwrap();
    assert_eq!(
        payload::encode(&arena_decoded, &value_decoded).unwrap(),
        json_expect
    );
    assert_eq!(
        arena_decoded.typ(&value_decoded).as_ref(),
        &typ::TypKind::Text
    );
    assert_eq!(arena_decoded.span(&value_decoded), &span_at(92));
    let values = get::tuple(&arena_decoded, &value_decoded).unwrap();
    assert!(get::bool(&arena_decoded, &values[0]).unwrap());
    assert_eq!(get::text(&arena_decoded, &values[1]).unwrap(), "saved");
    assert_eq!(arena_decoded.typ(&values[0]).as_ref(), &typ::TypKind::Bool);
    assert_eq!(arena_decoded.typ(&values[1]).as_ref(), &typ::TypKind::Text);
    assert_eq!(arena_decoded.span(&values[0]), &span_at(90));
    assert_eq!(arena_decoded.span(&values[1]), &span_at(91));
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
        .serialize_state(
            &mut serde_json::Serializer::new(&mut bytes),
            &payload::EncodeContext::new(&arena, payload::Encoding::ArenaIndependent),
        )
        .unwrap();
    let mut arena_decoded = ValueArena::new();
    let mut deserializer = serde_json::Deserializer::from_slice(&bytes);
    let value_decoded = Value::deserialize_state(
        &mut payload::DecodeContext::new(&mut arena_decoded, payload::Encoding::ArenaIndependent),
        &mut deserializer,
    )
    .unwrap();
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

#[test]
fn test_native_payload_restores_nested_values_on_small_stack() {
    std::thread::Builder::new()
        .stack_size(128 * 1024)
        .spawn(|| {
            let mut arena = ValueArena::new();
            let mut value = make::bool(&mut arena, true, Span::default()).unwrap();
            for _ in 0..256 {
                value = make::opt(
                    &mut arena,
                    typ::TypKind::Bool.into(),
                    Some(value),
                    Span::default(),
                )
                .unwrap();
            }
            let json_value = payload::encode(&arena, &value).unwrap();
            let mut arena_decoded = ValueArena::new();
            let mut value_decoded: Value =
                payload::decode(&mut arena_decoded, &json_value).unwrap();
            for _ in 0..256 {
                value_decoded = get::opt(&arena_decoded, &value_decoded).unwrap().unwrap();
            }
            assert!(get::bool(&arena_decoded, &value_decoded).unwrap());

            // Drop the JSON iteratively to isolate codec stack usage
            let mut jsons = vec![json_value];
            while let Some(json_value) = jsons.pop() {
                match json_value {
                    serde_json::Value::Array(jsons_inner) => jsons.extend(jsons_inner),
                    serde_json::Value::Object(jsons_fields) => {
                        jsons.extend(jsons_fields.into_values());
                    }
                    _ => {}
                }
            }
        })
        .unwrap()
        .join()
        .unwrap();
}

#[test]
fn test_native_payload_restores_recursive_types_and_mixfix_on_small_stack() {
    let mut jsons = std::thread::Builder::new()
        .stack_size(128 * 1024)
        .spawn(|| {
            let mut jsons = Vec::new();
            for encoding in [
                payload::Encoding::ArenaIndependent,
                payload::Encoding::ArenaRelative,
            ] {
                let mut arena = ValueArena::new();
                let mut typ = typ::make::bool();
                for _ in 0..256 {
                    typ = typ::make::opt(typ);
                }
                let json_typ = payload::encode_with(&arena, encoding, &typ).unwrap();
                let mut typ_decoded: typ::Typ =
                    payload::decode_with(&mut arena, encoding, &json_typ).unwrap();
                for _ in 0..256 {
                    let typ::TypKind::Iter(typ_inner, _) = typ_decoded.node else {
                        panic!("expected optional type");
                    };
                    typ_decoded = *typ_inner;
                    let typ::TypKind::Iter(typ_inner, _) = typ.node else {
                        panic!("expected original optional type");
                    };
                    typ = *typ_inner;
                }
                assert!(matches!(typ_decoded.node, typ::TypKind::Bool));

                let atom = p4spec_rust::phrase!(node: Atom::Arrow, span: Span::default());
                let mut mixfix = Mixfix::<Value>::Atom(atom.clone());
                for _ in 0..256 {
                    mixfix = Mixfix::Infix(
                        Box::new(mixfix),
                        atom.clone(),
                        Box::new(Mixfix::Atom(atom.clone())),
                    );
                }
                let json_mixfix = payload::encode_with(&arena, encoding, &mixfix).unwrap();
                let mut mixfix_decoded: Mixfix<Value> =
                    payload::decode_with(&mut arena, encoding, &json_mixfix).unwrap();
                for _ in 0..256 {
                    let Mixfix::Infix(mixfix_l, _, _) = mixfix_decoded else {
                        panic!("expected infix notation");
                    };
                    mixfix_decoded = *mixfix_l;
                    let Mixfix::Infix(mixfix_l, _, _) = mixfix else {
                        panic!("expected original infix notation");
                    };
                    mixfix = *mixfix_l;
                }
                assert!(matches!(mixfix_decoded, Mixfix::Atom(atom) if atom.node == Atom::Arrow));
                jsons.extend([json_typ, json_mixfix]);
            }
            jsons
        })
        .unwrap()
        .join()
        .unwrap();

    while let Some(json_value) = jsons.pop() {
        match json_value {
            serde_json::Value::Array(jsons_inner) => jsons.extend(jsons_inner),
            serde_json::Value::Object(jsons_fields) => jsons.extend(jsons_fields.into_values()),
            _ => {}
        }
    }
}

#[test]
fn test_native_payload_restores_opaque_objects_and_interned_cases_on_small_stack() {
    std::thread::Builder::new()
        .stack_size(128 * 1024)
        .spawn(|| {
            for opaque in [true, false] {
                // Construction and destruction are outside the codec under test
                let (arena, value) = stacker::grow(32 * 1024 * 1024, || {
                    let mut arena = ValueArena::new();
                    let value = if opaque {
                        let mut json_value = json!(true);
                        for _ in 0..512 {
                            json_value = json!({"nested": json_value});
                        }
                        make::external(
                            &mut arena,
                            typ::TypKind::Bool.into(),
                            json_value.into(),
                            Span::default(),
                        )
                        .unwrap()
                    } else {
                        let atom = p4spec_rust::phrase!(node: Atom::Arrow, span: Span::default());
                        let mut mixfix = Mixfix::Atom(atom.clone());
                        for _ in 0..512 {
                            mixfix = Mixfix::Infix(
                                Box::new(mixfix),
                                atom.clone(),
                                Box::new(Mixfix::Atom(atom.clone())),
                            );
                        }
                        make::case(
                            &mut arena,
                            typ::TypKind::Bool.into(),
                            mixfix,
                            Span::default(),
                        )
                        .unwrap()
                    };
                    (arena, value)
                });
                let json_value = payload::encode(&arena, &value).unwrap();
                let mut arena_decoded = ValueArena::new();
                let value_decoded: Value =
                    payload::decode(&mut arena_decoded, &json_value).unwrap();
                let value_repeated: Value =
                    payload::decode(&mut arena_decoded, &json_value).unwrap();
                assert_eq!(value_decoded.node, value_repeated.node);
                if opaque {
                    let mut json_inner = get::external(&arena_decoded, &value_decoded)
                        .unwrap()
                        .as_ref();
                    for _ in 0..512 {
                        json_inner = &json_inner["nested"];
                    }
                    assert_eq!(json_inner, &json!(true));
                } else {
                    let mut mixfix = get::case(&arena_decoded, &value_decoded).unwrap();
                    for _ in 0..512 {
                        let Mixfix::Infix(mixfix_l, _, _) = mixfix else {
                            panic!("expected infix notation")
                        };
                        mixfix = mixfix_l;
                    }
                    assert!(matches!(mixfix, Mixfix::Atom(atom) if atom.node == Atom::Arrow));
                }
                stacker::grow(32 * 1024 * 1024, || {
                    drop((arena, arena_decoded, json_value))
                });
            }
        })
        .unwrap()
        .join()
        .unwrap();
}
