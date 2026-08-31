use p4spec_rust::wire::ocaml::lang::sl;
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

fn note_phrase(it: Value, note: Value) -> Value {
    json!({"it": it, "note": note, "at": region()})
}

fn id(name: &str) -> Value {
    phrase(json!(name))
}

fn typ(kind: Value) -> Value {
    phrase(kind)
}

fn bool_typ() -> Value {
    typ(json!(["BoolT"]))
}

fn bool_exp(value: bool) -> Value {
    note_phrase(json!(["BoolE", value]), json!(["BoolT"]))
}

fn int_exp(value: &str) -> Value {
    note_phrase(json!(["NumE", ["Int", value]]), json!(["NumT", ["IntT"]]))
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

fn instruction(kind: Value, iid: i64) -> Value {
    note_phrase(kind, json!({"iid": iid}))
}

#[test]
fn test_sl_spec_codec_covers_all_sl_only_variant_families() {
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
                    [["SubG", bool_typ(), ["SkipSC"]], []],
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
