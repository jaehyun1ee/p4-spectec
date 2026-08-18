use p4spec_rust::{
    domain::external_data::ExternalData,
    wire::ocaml::{
        DecodeError, EncodeError,
        lang::{il, sl},
    },
};
use serde_json::{Value, json};

fn position(line: i64, column: i64) -> Value {
    json!({"file": "spec/test.watsup", "line": line, "column": column})
}

fn region() -> Value {
    json!({"left": position(1, 2), "right": position(1, 9)})
}

fn phrase(it: Value) -> Value {
    json!({"it": it, "note": null, "at": region()})
}

fn noted_phrase(it: Value, note: Value) -> Value {
    json!({"it": it, "note": note, "at": region()})
}

fn id(name: &str) -> Value {
    phrase(json!(name))
}

fn atom(tag: &str) -> Value {
    phrase(json!(["Keyword", tag]))
}

fn typ(kind: Value) -> Value {
    phrase(kind)
}

fn bool_typ() -> Value {
    typ(json!(["BoolT"]))
}

fn bool_exp(value: bool) -> Value {
    noted_phrase(json!(["BoolE", value]), json!(["BoolT"]))
}

fn int_exp(value: &str) -> Value {
    noted_phrase(json!(["NumE", ["Int", value]]), json!(["NumT", ["IntT"]]))
}

fn input_notation() -> Value {
    phrase(json!(["Seq", [["Arg", bool_typ()]]]))
}

fn el_hint() -> Value {
    json!({
        "hintid": id("latex"),
        "hintexp": phrase(json!([
            "FuseE",
            phrase(json!(["HoleE", ["Num", 2]])),
            phrase(json!(["LatexE", "x"])),
        ])),
    })
}

fn value_json(kind: Value, typ: Value) -> Value {
    noted_phrase(kind, json!({"vid": 7, "typ": typ, "vhash": 11}))
}

fn instruction(kind: Value, iid: i64) -> Value {
    noted_phrase(kind, json!({"iid": iid}))
}

#[test]
fn il_value_codec_round_trips_ppx_shapes_and_large_numbers() {
    let json = value_json(
        json!([
            "StructV",
            [
                [
                    atom("COUNT"),
                    value_json(
                        json!(["NumV", ["Nat", "123456789012345678901234567890"]]),
                        json!(["NumT", ["NatT"]]),
                    ),
                ],
                [
                    atom("MAYBE"),
                    value_json(
                        json!([
                            "OptV",
                            value_json(json!(["TextV", "payload"]), json!(["TextT"])),
                        ]),
                        json!(["IterT", bool_typ(), ["Opt"]]),
                    ),
                ],
                [
                    atom("ABSENT"),
                    value_json(json!(["OptV", null]), json!(["IterT", bool_typ(), ["Opt"]]),),
                ],
            ],
        ]),
        json!(["TupleT", []]),
    );

    let value = il::ValueCodec::decode(&json).expect("decode IL value");
    assert_eq!(
        il::ValueCodec::encode(&value).expect("encode IL value"),
        json
    );
}

#[test]
fn bigint_decoder_accepts_integer_but_encoder_canonicalizes_to_string() {
    let integer_json = value_json(json!(["NumV", ["Int", 42]]), json!(["NumT", ["IntT"]]));
    let string_json = value_json(json!(["NumV", ["Int", "42"]]), json!(["NumT", ["IntT"]]));

    let from_integer = il::ValueCodec::decode(&integer_json).expect("decode integer bigint");
    let from_string = il::ValueCodec::decode(&string_json).expect("decode string bigint");

    assert_eq!(from_integer, from_string);
    assert_eq!(
        il::ValueCodec::encode(&from_integer).expect("encode bigint"),
        string_json
    );
}

#[test]
fn il_spec_codec_handles_nested_definitions_and_el_hint_closure() {
    let json = vec![
        phrase(json!([
            "TypD",
            id("Pair"),
            [phrase(json!("a"))],
            phrase(json!([
                "VariantT",
                [[
                    phrase(json!(["Arg", bool_typ()])),
                    phrase(json!([id("Pair"), [phrase(json!(["BoolT"]))]])),
                    [el_hint()],
                ]],
            ])),
            [el_hint()],
        ])),
        phrase(json!([
            "RelD",
            id("step"),
            input_notation(),
            [0, 2],
            [phrase(json!([
                id("step"),
                [phrase(json!([
                    id("step"),
                    ["Arg", bool_exp(true)],
                    [phrase(json!(["IfPr", bool_exp(true)]))],
                ]))],
            ]))],
            phrase(json!([
                id("step"),
                phrase(json!([
                    id("step"),
                    ["Arg", bool_exp(false)],
                    [phrase(json!(["DebugPr", bool_exp(false)]))],
                ])),
            ])),
            [el_hint()],
        ])),
    ];
    let json = Value::Array(json);

    let spec = il::SpecCodec::decode(&json).expect("decode IL spec");
    assert_eq!(spec.len(), 2);
    assert_eq!(il::SpecCodec::encode(&spec).expect("encode IL spec"), json);
}

#[test]
fn sl_spec_codec_covers_all_sl_only_variant_families() {
    let rel_signature = json!([input_notation(), [0, 1]]);
    let iterexp = json!([["List"], [[id("xs"), bool_typ(), [["Opt"]]]]]);
    let iterinstr = json!([
        ["List"],
        [[id("x"), bool_typ(), []]],
        [[id("y"), bool_typ(), [["Opt"]]]]
    ]);
    let empty_block: Vec<Value> = Vec::new();

    let all_instructions = vec![
        instruction(
            json!(["IfI", bool_exp(true), [iterexp.clone()], [], false]),
            1,
        ),
        instruction(
            json!([
                "HoldI",
                id("r"),
                ["Arg", bool_exp(true)],
                [iterexp.clone()],
                ["BothH", [], []],
            ]),
            2,
        ),
        instruction(
            json!([
                "CaseI",
                int_exp("5"),
                [
                    [["BoolG", true], []],
                    [["CmpG", ["EqOp"], ["BoolT"], bool_exp(false)], []],
                    [["SubG", bool_typ()], []],
                    [["MatchG", ["ListP", ["Fixed", 3]]], []],
                    [["MemG", bool_exp(true)], []],
                ],
                true,
            ]),
            3,
        ),
        instruction(
            json!(["GroupI", id("group"), rel_signature.clone(), [], []]),
            4,
        ),
        instruction(
            json!([
                "LetI",
                bool_exp(true),
                bool_exp(false),
                [iterinstr.clone()],
                [],
            ]),
            5,
        ),
        instruction(
            json!([
                "RuleI",
                id("r"),
                ["Arg", bool_exp(true)],
                [1, 0],
                [iterinstr],
                [],
            ]),
            6,
        ),
        instruction(json!(["ResultI", rel_signature.clone(), []]), 7),
        instruction(json!(["ReturnI", bool_exp(true)]), 8),
        instruction(
            json!([
                "DebugI",
                bool_exp(false),
                instruction(json!(["ReturnI", bool_exp(true)]), 10),
            ]),
            9,
        ),
    ];

    let sl_params = vec![
        phrase(json!(["ExpP", bool_typ(), bool_exp(true)])),
        phrase(json!(["DefP", id("f"), [], [], bool_typ()])),
    ];
    let json = json!([
        phrase(json!(["ExternTypD", id("T"), [el_hint()]])),
        phrase(json!([
            "TypD",
            id("U"),
            [],
            phrase(json!(["PlainT", bool_typ()])),
            [],
        ])),
        phrase(json!(["VarD", id("v"), bool_typ(), []])),
        phrase(json!([
            "ExternRelD",
            [
                id("external"),
                rel_signature.clone(),
                [bool_exp(true)],
                [el_hint()]
            ],
        ])),
        phrase(json!([
            "RelD",
            [
                id("relation"),
                rel_signature.clone(),
                [bool_exp(false)],
                all_instructions,
                empty_block,
                [el_hint()],
            ],
        ])),
        phrase(json!([
            "ExternDecD",
            [id("extern_f"), [], sl_params.clone(), bool_typ(), []],
        ])),
        phrase(json!([
            "BuiltinDecD",
            [id("builtin_f"), [], sl_params.clone(), bool_typ(), []],
        ])),
        phrase(json!([
            "TableDecD",
            [
                id("table_f"),
                sl_params.clone(),
                bool_typ(),
                [[[bool_exp(true)], bool_exp(false), []]],
                [],
            ],
        ])),
        phrase(json!([
            "FuncDecD",
            [
                id("defined_f"),
                [],
                sl_params,
                bool_typ(),
                [instruction(json!(["ReturnI", bool_exp(true)]), 11)],
                null,
                [],
            ],
        ])),
    ]);

    let spec = sl::SpecCodec::decode(&json).expect("decode SL spec");
    assert_eq!(spec.len(), 9);
    assert_eq!(sl::SpecCodec::encode(&spec).expect("encode SL spec"), json);
}

#[test]
fn public_codecs_reject_unknown_tags_and_malformed_shapes() {
    let malformed = [
        json!({}),
        json!([phrase(json!(["VarD", id("v")]))]),
        json!([{"it": ["VarD", id("v"), bool_typ(), []], "at": region()}]),
        json!([phrase(json!(["UnknownDef"]))]),
    ];

    for json in malformed {
        assert!(il::SpecCodec::decode(&json).is_err(), "accepted {json}");
    }

    let error = il::ValueCodec::decode(&value_json(json!(["UnknownValue"]), json!(["BoolT"])))
        .expect_err("reject unknown value tag");
    assert!(matches!(error, DecodeError::UnknownVariant(_)));

    let malformed_sl = json!([phrase(json!([
        "RelD",
        [
            id("r"),
            [input_notation(), [0]],
            [],
            [instruction(json!(["IfI"]), 1)],
            null,
            []
        ],
    ]))]);
    assert!(sl::SpecCodec::decode(&malformed_sl).is_err());
}

#[test]
fn external_json_round_trips_or_returns_an_explicit_encode_error() {
    let json = value_json(
        json!([
            "ExternV",
            {"array": [null, true, -4, 1.25, "text"], "nested": {"ok": false}},
        ]),
        json!(["BoolT"]),
    );
    let mut value = il::ValueCodec::decode(&json).expect("decode external JSON");
    assert_eq!(
        il::ValueCodec::encode(&value).expect("encode external JSON"),
        json
    );

    let unsupported = [
        ExternalData::Float(f64::NAN),
        ExternalData::Tuple(vec![ExternalData::Null]),
        ExternalData::Variant("Tag".into(), None),
        ExternalData::Assoc(vec![
            ("duplicate".into(), ExternalData::Null),
            ("duplicate".into(), ExternalData::Bool(true)),
        ]),
        ExternalData::Intlit("123456789012345678901234567890".into()),
    ];

    for external in unsupported {
        value.it = p4spec_rust::lang::il::ast::ValueKind::ExternV(external);
        let error = il::ValueCodec::encode(&value).expect_err("reject non-standard JSON");
        assert!(matches!(error, EncodeError::UnsupportedExternalData(_)));
    }
}
