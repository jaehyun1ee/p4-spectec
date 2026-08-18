use p4spec_rust::wire::{Envelope, SL_SCHEMA, VALUE_SCHEMA, WireError};
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
