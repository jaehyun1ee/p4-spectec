use std::collections::HashSet;

use p4spec_rust::wire::ocaml::{
    DecodeError,
    lang::il::{SpecCodec, ValueCodec, ValueEnvelopeCodec},
};
use serde_json::{Value, json};

fn position(line: i64, column: i64) -> Value {
    json!({"file": "spec/nested.watsup", "line": line, "column": column})
}

fn phrase(it: Value) -> Value {
    json!({
        "it": it,
        "note": null,
        "at": {"left": position(1, 2), "right": position(1, 9)}
    })
}

fn note_phrase(it: Value, note: Value) -> Value {
    json!({
        "it": it,
        "note": note,
        "at": {"left": position(1, 2), "right": position(1, 9)}
    })
}

fn id(name: &str) -> Value {
    phrase(json!(name))
}

fn bool_typ() -> Value {
    phrase(json!(["BoolT"]))
}

fn bool_exp(value: bool) -> Value {
    note_phrase(json!(["BoolE", value]), json!(["BoolT"]))
}

fn bool_value(value: bool, vid: i64) -> Value {
    note_phrase(
        json!(["BoolV", value]),
        json!({"vid": vid, "typ": ["BoolT"], "vhash": 0}),
    )
}

fn natural_value(value: &str, vid: i64) -> Value {
    note_phrase(
        json!(["NumV", ["Nat", value]]),
        json!({"vid": vid, "typ": ["NumT", ["NatT"]], "vhash": 0}),
    )
}

fn tuple_value() -> Value {
    note_phrase(
        json!(["TupleV", [bool_value(true, 41), bool_value(false, 42)]]),
        json!({
            "vid": 43,
            "typ": ["TupleT", [bool_typ(), bool_typ()]],
            "vhash": 0
        }),
    )
}

fn collect_vids(value: &Value, vids: &mut Vec<i64>) {
    match value {
        Value::Array(values) => {
            for value in values {
                collect_vids(value, vids);
            }
        }
        Value::Object(fields) => {
            if let Some(vid) = fields
                .get("note")
                .and_then(Value::as_object)
                .and_then(|note| note.get("vid"))
                .and_then(Value::as_i64)
            {
                vids.push(vid);
            }
            for value in fields.values() {
                collect_vids(value, vids);
            }
        }
        _ => {}
    }
}

fn hint() -> Value {
    json!({
        "hintid": id("latex"),
        "hintexp": phrase(json!(["LatexE", "x"]))
    })
}

#[test]
fn whole_spec_roundtrip_preserves_relation_rules() {
    let notation = phrase(json!(["Seq", [["Arg", bool_typ()]]]));
    let rule = || {
        phrase(json!([
            id("step"),
            ["Arg", bool_exp(true)],
            [phrase(json!(["IfPr", bool_exp(true)]))]
        ]))
    };
    let rule_group = phrase(json!([id("step"), [rule()]]));
    let distinguished = phrase(json!([id("step"), rule()]));
    let spec = json!([phrase(json!([
        "RelD",
        id("step"),
        notation,
        [0, 1],
        [rule_group],
        distinguished,
        [hint()]
    ]))]);

    let decoded = SpecCodec::decode(&spec).expect("decode complete IL spec");
    assert_eq!(
        SpecCodec::encode(&decoded).expect("encode complete IL spec"),
        spec
    );
}

#[test]
fn repeated_value_encodes_are_deterministic() {
    let value = ValueCodec::decode(&tuple_value()).expect("decode nested IL value");

    let first = ValueCodec::encode(&value).expect("encode nested IL value");
    let second = ValueCodec::encode(&value).expect("encode nested IL value again");

    assert_eq!(second, first);
}

#[test]
fn separate_value_encode_operations_are_independent() {
    let value = ValueCodec::decode(&tuple_value()).expect("decode nested IL value");
    let other = ValueCodec::decode(&bool_value(false, 91)).expect("decode other IL value");

    let first = ValueEnvelopeCodec::encode(&value).expect("encode nested IL value envelope");
    ValueCodec::encode(&other).expect("encode independent IL value");
    let second = ValueEnvelopeCodec::encode(&value).expect("encode nested IL value envelope again");

    assert_eq!(second, first);
}

#[test]
fn nested_value_encode_assigns_unique_identifiers() {
    let value = ValueCodec::decode(&tuple_value()).expect("decode nested IL value");
    let encoded = ValueCodec::encode(&value).expect("encode nested IL value");
    let mut vids = Vec::new();

    collect_vids(&encoded, &mut vids);

    assert_eq!(encoded["note"]["vhash"], 0);
    assert_eq!(vids.len(), 3);
    assert_eq!(vids.iter().copied().collect::<HashSet<_>>().len(), 3);
}

#[test]
fn natural_value_wire_preserves_ocaml_json_spelling() {
    let wire = natural_value("123456789012345678901234567890", 77);
    let value = ValueCodec::decode(&wire).expect("decode natural IL value");

    assert_eq!(
        ValueCodec::encode(&value).expect("encode natural IL value"),
        natural_value("123456789012345678901234567890", 0)
    );
}

#[test]
fn natural_value_wire_rejects_negative_payloads() {
    let error =
        ValueCodec::decode(&natural_value("-1", 9)).expect_err("reject negative natural IL value");

    assert_eq!(error, DecodeError::Expected("non-negative natural number"));
}
