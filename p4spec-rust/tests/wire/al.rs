use p4spec_rust::wire::ocaml::lang::al::SpecCodec;
use serde_json::{Value, json};

fn phrase(it: Value) -> Value {
    json!({
        "it": it,
        "note": null,
        "at": {
            "left": {"file": "rules.watsup", "line": 3, "column": 1},
            "right": {"file": "rules.watsup", "line": 5, "column": 7}
        }
    })
}

fn id(name: &str) -> Value {
    phrase(json!(name))
}
fn typ() -> Value {
    phrase(json!(["BoolT"]))
}
fn exp(value: bool) -> Value {
    json!({"it": ["BoolE", value], "note": ["BoolT"], "at": phrase(json!(null))["at"].clone()})
}

#[test]
fn test_whole_spec_roundtrip_preserves_rule_paths_and_tables() {
    let rule_match = json!([
        [exp(true)],
        [exp(false)],
        [phrase(json!(["IfPr", exp(true)]))]
    ]);
    let rule_path = json!([
        id("step"),
        [phrase(json!(["DebugPr", exp(false)]))],
        [exp(true)]
    ]);
    let group = phrase(json!([id("step"), rule_match, [rule_path]]));
    let notation = phrase(json!(["Seq", [["Arg", typ()]]]));
    let row = phrase(json!([
        [exp(true)],
        [phrase(json!(["ExpA", exp(false)]))],
        exp(true),
        []
    ]));
    let spec = json!([
        phrase(json!([
            "RelD",
            id("step"),
            notation,
            [0],
            [group],
            null,
            []
        ])),
        phrase(json!(["TableDecD", id("table"), [], typ(), [row], []]))
    ]);

    let decoded = SpecCodec::decode(&spec).expect("decode complete AL spec");
    assert_eq!(
        SpecCodec::encode(&decoded).expect("encode complete AL spec"),
        spec
    );
}
