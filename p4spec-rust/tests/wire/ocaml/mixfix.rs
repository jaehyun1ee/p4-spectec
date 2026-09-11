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
    let mut spans = Vec::new();
    let mixop_tokens = mixop.map_span(|span| {
        let index = u8::try_from(spans.len()).unwrap();
        spans.push(span);
        index
    });
    let mixop_restored = mixop_tokens.map_span(|index| spans[usize::from(index)].clone());

    assert_eq!(mixop_restored.arity(), 2);
    assert_eq!(MixopCodec::encode(&mixop_restored), json);
}
