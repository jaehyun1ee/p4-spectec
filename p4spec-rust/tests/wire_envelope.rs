use p4spec_rust::wire::{
    AL_SCHEMA, EL_SCHEMA, Envelope, IL_SCHEMA, PL_SCHEMA, SL_SCHEMA, WireError,
};
use serde_json::{Value, json};

#[test]
fn stage_envelopes_round_trip_with_registered_schema_and_kind() {
    let cases = [
        (Envelope::el(json!({"stage": "el"})), EL_SCHEMA, "el"),
        (Envelope::il(json!({"stage": "il"})), IL_SCHEMA, "il"),
        (Envelope::al(json!({"stage": "al"})), AL_SCHEMA, "al"),
        (Envelope::sl(json!({"stage": "sl"})), SL_SCHEMA, "sl"),
        (Envelope::pl(json!({"stage": "pl"})), PL_SCHEMA, "pl"),
    ];

    for (envelope, schema, kind) in cases {
        let bytes = serde_json::to_vec(&envelope).unwrap();
        let decoded: Envelope<Value> = Envelope::from_slice(&bytes).unwrap();

        assert_eq!(decoded.schema(), schema);
        assert_eq!(decoded.kind(), kind);
    }
}

#[test]
fn registered_stage_schema_rejects_another_stage_kind() {
    let input = br#"{
        "schema":"p4spectec.el.v1",
        "kind":"il",
        "payload":[]
    }"#;

    let error = Envelope::<Value>::from_slice(input).unwrap_err();
    assert!(matches!(error, WireError::SchemaKindMismatch { .. }));
}
