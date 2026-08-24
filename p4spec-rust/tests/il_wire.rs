use p4spec_rust::wire::ocaml::lang::il::SpecCodec;
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

fn noted_phrase(it: Value, note: Value) -> Value {
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
    noted_phrase(json!(["BoolE", value]), json!(["BoolT"]))
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
