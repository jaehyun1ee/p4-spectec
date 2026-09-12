use p4spec_rust::{
    lang::{
        common::{
            notation::{atom::Atom, mixfix::Mixfix},
            source::{Position, Span},
        },
        data::{
            typ,
            value::{ValueArena, get, make},
        },
    },
    phrase,
    wire::ocaml::lang::il::{ValueCodec, ValueEnvelopeCodec},
};

fn span(line: i64) -> Span {
    Span::new(
        Position::new("value.spec", line, 2),
        Position::new("value.spec", line, 7),
    )
}

#[test]
fn test_value_wire_expands_shared_bodies_with_distinct_annotations() {
    let mut arena = ValueArena::new();
    let child = make::bool(&mut arena, true, span(11)).unwrap();
    let child_relocated = arena.update_span(child, span(13)).unwrap();
    let case = Mixfix::Brack(
        phrase!(node: Atom::LParen, span: span(17)),
        Box::new(Mixfix::Infix(
            Box::new(Mixfix::Arg(child)),
            phrase!(node: Atom::Arrow, span: span(19)),
            Box::new(Mixfix::Seq(vec![
                Mixfix::Atom(phrase!(node: Atom::Keyword("tail".to_owned()), span: span(23))),
                Mixfix::Arg(child_relocated),
            ])),
        )),
        phrase!(node: Atom::RParen, span: span(29)),
    );
    let value = make::case(&mut arena, typ::TypKind::Bool.into(), case, span(31)).unwrap();
    let field = phrase!(node: Atom::keyword("field"), span: span(37));
    let structure = make::structure(
        &mut arena,
        typ::TypKind::Bool.into(),
        vec![(field, value)],
        span(41),
    )
    .unwrap();
    let id = phrase!(node: "function".to_owned(), span: span(43));
    let func = make::func(&mut arena, id, vec![], vec![], typ::make::bool(), span(47)).unwrap();
    let tuple = make::tuple(
        &mut arena,
        typ::make::tuple(vec![]).node.clone().into(),
        vec![structure, func],
        span(53),
    )
    .unwrap();
    let json = ValueCodec::encode(&arena, &tuple).unwrap();
    let mut json_located = json.clone();
    for (path, line) in [
        ("/it/1/0/it/1/0/0/at", 37),
        ("/it/1/0/it/1/0/1/it/1/1/at", 17),
        ("/it/1/0/it/1/0/1/it/1/2/2/at", 19),
        ("/it/1/0/it/1/0/1/it/1/2/3/1/0/1/at", 23),
        ("/it/1/0/it/1/0/1/it/1/3/at", 29),
        ("/it/1/1/it/1/at", 43),
    ] {
        let region = json_located.pointer_mut(path).unwrap();
        assert_eq!(
            *region,
            p4spec_rust::wire::ocaml::source::encode_region(&span(line))
        );
        *region = p4spec_rust::wire::ocaml::source::encode_region(&span(line + 100));
    }
    let tuple_located = ValueCodec::decode(&mut arena, &json_located).unwrap();
    assert_ne!(tuple_located.node, tuple.node);
    assert_eq!(arena.canon_id(&tuple_located), arena.canon_id(&tuple));
    assert_eq!(
        ValueCodec::encode(&arena, &tuple_located).unwrap(),
        json_located
    );
    let envelope_located = serde_json::to_vec(&serde_json::json!({
        "schema": p4spec_rust::wire::VALUE_SCHEMA,
        "kind": "value",
        "payload": json_located,
    }))
    .unwrap();
    let tuple_envelope = ValueEnvelopeCodec::decode(&mut arena, &envelope_located).unwrap();
    assert_eq!(
        arena.canon_id(&tuple_envelope),
        arena.canon_id(&tuple_located)
    );
    assert_eq!(
        ValueCodec::encode(&arena, &tuple_envelope).unwrap(),
        ValueCodec::encode(&arena, &tuple_located).unwrap()
    );
    let envelope = ValueEnvelopeCodec::encode(&arena, &tuple).unwrap();
    let mut arena_decoded = ValueArena::new();
    let tuple_decoded = ValueEnvelopeCodec::decode(&mut arena_decoded, &envelope).unwrap();
    assert_eq!(
        ValueCodec::encode(&arena_decoded, &tuple_decoded).unwrap(),
        json
    );
    assert_eq!(
        ValueEnvelopeCodec::encode(&arena_decoded, &tuple_decoded).unwrap(),
        envelope
    );
    let values = get::tuple(&arena_decoded, &tuple_decoded).unwrap();
    let fields = get::structure(&arena_decoded, &values[0]).unwrap();
    assert_eq!(fields[0].0.node, Atom::keyword("field"));
    assert_eq!(fields[0].0.span, span(37));
    let case = get::case(&arena_decoded, &fields[0].1).unwrap();
    assert_eq!(
        case.atoms()
            .into_iter()
            .map(|atom| &atom.node)
            .collect::<Vec<_>>(),
        vec![
            &Atom::LParen,
            &Atom::Arrow,
            &Atom::keyword("tail"),
            &Atom::RParen
        ]
    );
    let args = case.args();
    assert_eq!(args[0].node, args[1].node);
    assert_eq!(arena_decoded.span(args[0]), &span(11));
    assert_eq!(arena_decoded.span(args[1]), &span(13));
    assert_eq!(
        get::func(&arena_decoded, &values[1]).unwrap().node,
        "function"
    );
}

#[test]
#[should_panic(expected = "extern payloads are not supported by OCaml wire")]
fn test_value_wire_rejects_extern_encoding() {
    let mut arena = ValueArena::new();
    let value = make::external(
        &mut arena,
        typ::TypKind::Bool.into(),
        serde_json::Value::Null,
        Span::default(),
    )
    .unwrap();
    let _ = ValueCodec::encode(&arena, &value);
}

#[test]
#[should_panic(expected = "extern payloads are not supported by OCaml wire")]
fn test_value_wire_rejects_extern_decoding() {
    let mut arena = ValueArena::new();
    let value = make::bool(&mut arena, true, Span::default()).unwrap();
    let mut json = ValueCodec::encode(&arena, &value).unwrap();
    json["it"] = serde_json::json!(["ExternV", null]);
    let _ = ValueCodec::decode(&mut arena, &json);
}

#[test]
fn test_value_envelope_rejects_lossy_or_extended_json() {
    let mut arena = ValueArena::new();
    let value = make::bool(&mut arena, true, Span::default()).unwrap();
    let payload = ValueCodec::encode(&arena, &value).unwrap();
    for text in [
        "null",
        "NaN",
        "Infinity",
        "-Infinity",
        "(1,2)",
        "<\"Tag\":1>",
        "18446744073709551616",
        "-9223372036854775809",
        "1e400",
        "1.5",
        r#"{"key":1,"key":2}"#,
        r#"{"key":1,"\u006bey":2}"#,
    ] {
        let input = format!(
            r#"{{"schema":"p4spectec.value.v1","kind":"value","payload":{payload},"extra":{text}}}"#
        );
        // Null is valid even in an unused field; unsupported tokens must not be ignored
        if text == "null" {
            assert!(ValueEnvelopeCodec::decode(&mut arena, input.as_bytes()).is_ok());
        } else {
            assert!(
                ValueEnvelopeCodec::decode(&mut arena, input.as_bytes()).is_err(),
                "{text}"
            );
        }
    }
    for input in [
        format!(
            r#"{{"schema":"p4spectec.value.v1","schema":"p4spectec.value.v1","kind":"value","payload":{payload}}}"#
        ),
        format!(r#"{{"schema":"unknown","kind":"value","payload":{payload}}}"#),
        format!(r#"{{"schema":"p4spectec.value.v1","kind":"sl","payload":{payload}}}"#),
        format!(r#"{{"schema":"p4spectec.value.v1","kind":"value","payload":{payload}}} null"#),
    ] {
        assert!(
            ValueEnvelopeCodec::decode(&mut arena, input.as_bytes()).is_err(),
            "{input}"
        );
    }
}

#[test]
#[should_panic(expected = "extern payloads are not supported by OCaml wire")]
fn test_value_envelope_rejects_nested_extern_encoding() {
    let mut arena = ValueArena::new();
    let value = make::external(
        &mut arena,
        typ::TypKind::Bool.into(),
        serde_json::Value::Null,
        Span::default(),
    )
    .unwrap();
    let value = make::list(
        &mut arena,
        typ::TypKind::Bool.into(),
        vec![value],
        Span::default(),
    )
    .unwrap();
    let _ = ValueEnvelopeCodec::encode(&arena, &value);
}

#[test]
#[should_panic(expected = "extern payloads are not supported by OCaml wire")]
fn test_value_envelope_rejects_extern_decoding() {
    let mut arena = ValueArena::new();
    let value = make::bool(&mut arena, true, Span::default()).unwrap();
    let mut payload = ValueCodec::encode(&arena, &value).unwrap();
    payload["it"] = serde_json::json!(["ExternV", {"state": 1}]);
    let input = serde_json::to_vec(&serde_json::json!({
        "schema": "p4spectec.value.v1", "kind": "value", "payload": payload,
    }))
    .unwrap();
    let _ = ValueEnvelopeCodec::decode(&mut arena, &input);
}

#[test]
fn test_value_envelope_preserves_large_language_integers() {
    use num_bigint::BigInt;
    let mut arena = ValueArena::new();
    let int = BigInt::from(1_u8) << 256;
    let value = make::int(&mut arena, int, span(7)).unwrap();
    let input = ValueEnvelopeCodec::encode(&arena, &value).unwrap();
    let mut arena_decoded = ValueArena::new();
    let value_decoded = ValueEnvelopeCodec::decode(&mut arena_decoded, &input).unwrap();
    assert_eq!(
        get::num(&arena_decoded, &value_decoded).unwrap(),
        get::num(&arena, &value).unwrap()
    );
    assert_eq!(arena_decoded.span(&value_decoded), &span(7));
}
