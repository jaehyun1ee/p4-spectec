use p4spec_rust::wire::ocaml::lang::el::SpecCodec;
use serde_json::{Value, json};

fn at(it: Value) -> Value {
    json!({
        "it": it,
        "note": null,
        "at": {
            "left": {"file": "nested.p4s", "line": 2, "column": 3},
            "right": {"file": "nested.p4s", "line": 2, "column": 9}
        }
    })
}

#[test]
fn test_whole_spec_roundtrip_preserves_nested_definitions() {
    let id = |name| at(json!(name));
    let bool_typ = || at(json!(["BoolT"]));
    let bool_exp = || at(json!(["BoolE", true]));
    let spec = json!([
        at(json!([
            "TypD",
            id("flag"),
            [],
            at(json!(["PlainTD", bool_typ()])),
            []
        ])),
        at(json!([
            "FuncDefD",
            id("accept"),
            [],
            [at(json!(["ExpA", bool_exp()]))],
            bool_exp(),
            [at(json!(["IfPr", bool_exp()]))]
        ]))
    ]);

    let decoded = SpecCodec::decode(&spec).expect("decode complete EL spec");
    assert_eq!(
        SpecCodec::encode(&decoded).expect("encode complete EL spec"),
        spec
    );
}
