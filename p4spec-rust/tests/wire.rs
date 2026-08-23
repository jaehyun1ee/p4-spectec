use p4spec_rust::wire::{
    AL_SCHEMA, EL_SCHEMA, Envelope, IL_SCHEMA, PL_SCHEMA, SL_SCHEMA, VALUE_SCHEMA, WireError,
};
use serde_json::{Value, json};

#[test]
fn sl_envelope_round_trips() {
    let envelope = Envelope::sl(json!([]));
    let bytes = serde_json::to_vec(&envelope).expect("serialize SL envelope");
    let decoded: Envelope<Value> = Envelope::from_slice(&bytes).expect("deserialize SL envelope");

    assert_eq!(decoded.schema(), SL_SCHEMA);
    assert_eq!(decoded.kind(), "sl");
    assert_eq!(decoded.payload(), &json!([]));
}

#[test]
fn value_envelope_round_trips() {
    let envelope = Envelope::value(json!(["BoolV", true]));
    let bytes = serde_json::to_vec(&envelope).expect("serialize value envelope");
    let decoded: Envelope<Value> =
        Envelope::from_slice(&bytes).expect("deserialize value envelope");

    assert_eq!(decoded.schema(), VALUE_SCHEMA);
    assert_eq!(decoded.kind(), "value");
    assert_eq!(decoded.payload(), &json!(["BoolV", true]));
}

#[test]
fn stage_envelopes_round_trip_with_their_schema_and_kind() {
    let cases = [
        (Envelope::el(json!({"node": "el"})), EL_SCHEMA, "el"),
        (Envelope::il(json!({"node": "il"})), IL_SCHEMA, "il"),
        (Envelope::al(json!({"node": "al"})), AL_SCHEMA, "al"),
        (Envelope::pl(json!({"node": "pl"})), PL_SCHEMA, "pl"),
    ];

    for (envelope, schema, kind) in cases {
        let bytes = serde_json::to_vec(&envelope).expect("serialize stage envelope");
        let decoded: Envelope<Value> =
            Envelope::from_slice(&bytes).expect("deserialize stage envelope");

        assert_eq!(decoded.schema(), schema);
        assert_eq!(decoded.kind(), kind);
    }
}

#[test]
fn stage_schema_rejects_another_stage_kind() {
    let input = br#"{
        "schema":"p4spectec.el.v1",
        "kind":"il",
        "payload":{"node":"el"}
    }"#;

    let error = Envelope::<Value>::from_slice(input).expect_err("reject mismatched stage kind");
    assert!(matches!(error, WireError::SchemaKindMismatch { .. }));
}

#[test]
fn schema_and_kind_must_agree() {
    let input = br#"{
        "schema":"p4spectec.sl.v1",
        "kind":"value",
        "payload":[]
    }"#;

    let error = Envelope::<Value>::from_slice(input).expect_err("reject mismatch");
    assert!(matches!(error, WireError::SchemaKindMismatch { .. }));
}

#[test]
fn unknown_schema_is_rejected() {
    let input = br#"{
        "schema":"p4spectec.sl.v2",
        "kind":"sl",
        "payload":[]
    }"#;

    let error = Envelope::<Value>::from_slice(input).expect_err("reject schema");
    assert!(matches!(error, WireError::UnknownSchema(_)));
}

#[test]
fn malformed_json_is_rejected() {
    let error = Envelope::<Value>::from_slice(br#"{"schema"#).expect_err("reject JSON");
    assert!(matches!(error, WireError::Json(_)));
}

#[test]
fn deeply_nested_payload_deserializes() {
    let mut payload = String::from("null");
    for _ in 0..512 {
        payload = format!("[{payload}]");
    }
    let input = format!(r#"{{"schema":"{VALUE_SCHEMA}","kind":"value","payload":{payload}}}"#);

    Envelope::<Value>::from_slice(input.as_bytes()).expect("deserialize deep payload");
}

#[test]
fn ocaml_golden_envelopes_deserialize() {
    let sl_fixture = include_bytes!("../../p4spec/test/wire/minimal-sl.expected.json");
    let value_fixture = include_bytes!("../../p4spec/test/wire/minimal-value.expected.json");

    let sl = Envelope::<Value>::from_slice(sl_fixture).expect("deserialize OCaml SL fixture");
    let value =
        Envelope::<Value>::from_slice(value_fixture).expect("deserialize OCaml value fixture");

    assert_eq!(sl.schema(), SL_SCHEMA);
    assert_eq!(sl.kind(), "sl");
    assert_eq!(sl.payload(), &json!([]));
    assert_eq!(value.schema(), VALUE_SCHEMA);
    assert_eq!(value.kind(), "value");
    assert_eq!(value.payload()["note"]["typ"][0], "VarT");
}
