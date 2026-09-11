use p4spec_rust::wire::ocaml::mixfix::MixopCodec;
use serde_json::{Value, json};

fn atom(node: Value, line: i64) -> Value {
    json!({
        "it": node,
        "note": null,
        "at": {
            "left": {"file": "notation.spec", "line": line, "column": 2},
            "right": {"file": "notation.spec", "line": line, "column": 5},
        },
    })
}

#[test]
fn test_mixop_span_round_trip_preserves_ocaml_wire_layout() {
    let json = json!([
        "Brack",
        atom(json!(["LParen"]), 11),
        [
            "Infix",
            ["Arg", null],
            atom(json!(["Arrow"]), 13),
            [
                "Seq",
                [
                    ["Atom", atom(json!(["Keyword", "tail"]), 17)],
                    ["Arg", null],
                ],
            ],
        ],
        atom(json!(["RParen"]), 19),
    ]);
    let mixop = MixopCodec::decode(&json).unwrap();

    assert_eq!(mixop.arity(), 2);
    assert_eq!(MixopCodec::encode(&mixop), json);
}
