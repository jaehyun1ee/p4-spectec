use p4spec_rust::wire::ocaml::lang::pl::{ExpCodec, PathCodec};
use serde_json::{Value, json};

fn region() -> Value {
    json!({
        "left": {"file": "prose.watsup", "line": 7, "column": 2},
        "right": {"file": "prose.watsup", "line": 7, "column": 12}
    })
}

fn phrase(it: Value) -> Value {
    json!({"it": it, "note": null, "at": region()})
}
fn id(name: &str) -> Value {
    phrase(json!(name))
}
fn il_exp(value: bool) -> Value {
    json!({"it": ["BoolE", value], "note": ["BoolT"], "at": region()})
}

fn hints() -> Value {
    json!({
        "prose": ["SeqH", [["TextH", "before"], ["HoleH", ["Next"]]]],
        "prose_in": null,
        "prose_out": ["OtherH", phrase(json!(["LatexE", "x"]))],
        "prose_true": null,
        "prose_false": null,
        "prose_fields": ["left", "right"],
        "prose_input_exps": [il_exp(true)],
        "prose_output_exps": [il_exp(false)]
    })
}

fn exp(kind: Value) -> Value {
    json!({"node": {"it": kind, "note": ["BoolT"], "at": region()}, "hints": hints()})
}

#[test]
fn annotation_expression_and_path_roundtrip() {
    let root = json!({"it": ["RootP"], "note": ["BoolT"], "at": region()});
    let path = json!({"it": ["IdxP", root, exp(json!(["BoolE", true]))], "note": ["BoolT"], "at": region()});
    let update = exp(json!([
        "UpdE",
        exp(json!(["VarE", id("record")])),
        path,
        exp(json!([
            "CallE",
            id("make"),
            [],
            [phrase(json!(["ExpA", exp(json!(["BoolE", false]))]))]
        ]))
    ]));

    let decoded = ExpCodec::decode(&update).expect("decode annotated PL expression");
    assert_eq!(ExpCodec::encode(&decoded), update);

    let path = update["node"]["it"][2].clone();
    let decoded = PathCodec::decode(&path).expect("decode recursive PL path");
    assert_eq!(PathCodec::encode(&decoded), path);
}
