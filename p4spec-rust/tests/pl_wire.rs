use p4spec_rust::wire::ocaml::lang::pl::{ExpCodec, PathCodec, SpecCodec};
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
fn typ() -> Value {
    phrase(json!(["BoolT"]))
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

fn instr(kind: Value, iid: i64, fallthrough: Value) -> Value {
    json!({
        "node": {"it": kind, "note": {"iid": iid, "fallthrough": fallthrough}, "at": region()},
        "hints": hints()
    })
}

fn def(kind: Value) -> Value {
    json!({"node": phrase(kind), "hints": hints()})
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

#[test]
fn whole_spec_roundtrip_preserves_dispatch_and_groups() {
    let signature = json!([phrase(json!(["Seq", [["Arg", typ()]]])), [0]]);
    let returned = instr(
        json!(["TierI", ["ReturnI", exp(json!(["BoolE", true]))]]),
        2,
        json!(["FallNext"]),
    );
    let group = instr(
        json!([
            "TierI",
            ["GroupI", id("group"), id("step"), signature, [], [returned]]
        ]),
        1,
        json!(["FallGroup", id("next")]),
    );
    let row = json!([
        [],
        exp(json!(["BoolE", false])),
        [instr(
            json!(["TierI", ["BacktrackI", [[]]]]),
            3,
            Value::Null
        )]
    ]);
    let param = phrase(json!(["ExpP", typ(), exp(json!(["BoolE", true]))]));
    let spec = json!([
        def(json!(["ExternTypD", id("T")])),
        def(json!(["RelD", [id("step"), signature, [], [group], null]])),
        def(json!(["TableDecD", [id("table"), [param], typ(), [row]]])),
        def(json!([
            "FuncDecD",
            [
                id("f"),
                [],
                [],
                typ(),
                [instr(
                    json!(["TierI", ["ReturnI", exp(json!(["BoolE", true]))]]),
                    4,
                    json!(["FallFail"])
                )],
                []
            ]
        ]))
    ]);

    let decoded = SpecCodec::decode(&spec).expect("decode complete PL spec");
    assert_eq!(
        SpecCodec::encode(&decoded).expect("encode complete PL spec"),
        spec
    );
}

#[test]
fn decoded_relation_exposes_named_model_fields() {
    let signature = json!([phrase(json!(["Seq", [["Arg", typ()]]])), [0]]);
    let relation = def(json!(["RelD", [id("step"), signature, [], [], null]]));

    let spec = SpecCodec::decode(&json!([relation])).expect("decode PL relation");
    let p4spec_rust::lang::pl::ast::DefKind::RelD(relation) = &spec[0].node.kind else {
        panic!("expected a relation declaration");
    };

    assert_eq!(relation.id.node, "step");
    assert!(relation.inputs.is_empty());
    assert!(relation.block.is_empty());
    assert!(relation.else_block.is_none());
}

#[test]
fn decoded_group_exposes_named_dispatch_fields() {
    let signature = json!([phrase(json!(["Seq", [["Arg", typ()]]])), [0]]);
    let group = instr(
        json!([
            "TierI",
            ["GroupI", id("group"), id("step"), signature, [], []]
        ]),
        1,
        Value::Null,
    );
    let relation = def(json!([
        "RelD",
        [
            id("step"),
            json!([phrase(json!(["Seq", [["Arg", typ()]]])), [0]]),
            [],
            [group],
            null
        ]
    ]));

    let spec = SpecCodec::decode(&json!([relation])).expect("decode PL group");
    let p4spec_rust::lang::pl::ast::DefKind::RelD(relation) = &spec[0].node.kind else {
        panic!("expected a relation declaration");
    };
    let p4spec_rust::lang::pl::ast::InstrKind::TierI(
        p4spec_rust::lang::pl::ast::InstrDispatch::GroupI {
            group_id,
            relation_id,
            ..
        },
    ) = &relation.block[0].node.kind
    else {
        panic!("expected a dispatch group");
    };

    assert_eq!(group_id.node, "group");
    assert_eq!(relation_id.node, "step");
}
