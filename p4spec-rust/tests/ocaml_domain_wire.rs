use p4spec_rust::{
    domain::{atom::Atom, mixfix::Mixfix},
    wire::{
        Envelope,
        ocaml::{DecodeError, atom::AtomPhraseCodec, mixfix::MixopCodec},
    },
};
use serde_json::{Value, json};

fn position(file: &str, line: i64, column: i64) -> Value {
    json!({"file": file, "line": line, "column": column})
}

fn region() -> Value {
    json!({
        "left": position("spec/test.watsup", 2, 3),
        "right": position("spec/test.watsup", 2, 5)
    })
}

fn atom_phrase(atom: Value) -> Value {
    json!({"it": atom, "note": null, "at": region()})
}

#[test]
fn atom_phrase_uses_ppx_deriving_yojson_shape() {
    let json = atom_phrase(json!(["Keyword", "IF"]));
    let atom = AtomPhraseCodec::decode(&json).expect("decode atom phrase");

    assert_eq!(atom.node, Atom::Keyword("IF".into()));
    assert_eq!(AtomPhraseCodec::encode(&atom), json);
}

#[test]
fn every_atom_variant_uses_ocaml_array_encoding() {
    let variants = [
        json!(["Keyword", "WORD"]),
        json!(["Tag", "TAG"]),
        json!(["Operator", "+"]),
        json!(["Sub"]),
        json!(["Sup"]),
        json!(["Turnstile"]),
        json!(["Tilesturn"]),
        json!(["Arrow"]),
        json!(["ArrowSub"]),
        json!(["DoubleArrowSub"]),
        json!(["DoubleArrowLong"]),
        json!(["SqArrow"]),
        json!(["SqArrowStar"]),
        json!(["Dot"]),
        json!(["Dot2"]),
        json!(["Dot3"]),
        json!(["Semicolon"]),
        json!(["Colon"]),
        json!(["ColonEq"]),
        json!(["Tilde2"]),
        json!(["Backslash"]),
        json!(["LAngle"]),
        json!(["RAngle"]),
        json!(["LParen"]),
        json!(["RParen"]),
        json!(["LBrack"]),
        json!(["RBrack"]),
        json!(["LBrace"]),
        json!(["RBrace"]),
    ];

    for variant in variants {
        let phrase_json = atom_phrase(variant);
        let atom = AtomPhraseCodec::decode(&phrase_json).expect("decode atom variant");
        assert_eq!(AtomPhraseCodec::encode(&atom), phrase_json);
    }
}

#[test]
fn mixop_codec_matches_ocaml_custom_encoding() {
    let if_atom = atom_phrase(json!(["Keyword", "IF"]));
    let lparen = atom_phrase(json!(["LParen"]));
    let rparen = atom_phrase(json!(["RParen"]));
    let json = json!([
        "Seq",
        [
            ["Atom", if_atom],
            ["Arg", null],
            ["Brack", lparen, ["Arg", null], rparen]
        ]
    ]);

    let mixop = MixopCodec::decode(&json).expect("decode mixop");
    assert_eq!(mixop.arity(), 2);
    assert!(matches!(mixop, Mixfix::Seq(_)));
    assert_eq!(MixopCodec::encode(&mixop), json);
}

#[test]
fn deep_public_mixop_codec_uses_grown_stack() {
    const DEPTH: usize = 1_024;

    let mut json = Value::Array(vec![Value::String("Arg".into()), Value::Null]);
    for _ in 0..DEPTH {
        json = Value::Array(vec![Value::String("Seq".into()), Value::Array(vec![json])]);
    }
    let mixop = match MixopCodec::decode(&json) {
        Ok(mixop) => mixop,
        Err(error) => {
            std::mem::forget(json);
            panic!("decode deep mixop: {error}");
        }
    };
    let mut decoded = &mixop;
    let mut decoded_depth = 0;
    while let Mixfix::Seq(items) = decoded {
        let [item] = items.as_slice() else {
            break;
        };
        decoded = item;
        decoded_depth += 1;
    }
    let decoded_arg = matches!(decoded, Mixfix::Arg(()));

    let encoded = MixopCodec::encode(&mixop);
    let mut encoded_item = &encoded;
    let mut encoded_depth = 0;
    while let Some([Value::String(tag), Value::Array(items)]) =
        encoded_item.as_array().map(Vec::as_slice)
    {
        if tag != "Seq" {
            break;
        }
        let [item] = items.as_slice() else {
            break;
        };
        encoded_item = item;
        encoded_depth += 1;
    }
    let encoded_arg = encoded_item == &json!(["Arg", null]);

    std::mem::forget(json);
    std::mem::forget(mixop);
    std::mem::forget(encoded);

    assert_eq!(decoded_depth, DEPTH);
    assert!(decoded_arg);
    assert_eq!(encoded_depth, DEPTH);
    assert!(encoded_arg);
}

#[test]
fn unknown_ocaml_variant_is_rejected() {
    let error = AtomPhraseCodec::decode(&atom_phrase(json!(["UnknownAtom"])))
        .expect_err("reject unknown atom");

    assert!(matches!(error, DecodeError::UnknownVariant(_)));
}

#[test]
fn atom_codec_accepts_phrases_from_ocaml_value_golden() {
    fn is_atom_variant(tag: &str) -> bool {
        matches!(
            tag,
            "Keyword"
                | "Tag"
                | "Operator"
                | "Sub"
                | "Sup"
                | "Turnstile"
                | "Tilesturn"
                | "Arrow"
                | "ArrowSub"
                | "DoubleArrowSub"
                | "DoubleArrowLong"
                | "SqArrow"
                | "SqArrowStar"
                | "Dot"
                | "Dot2"
                | "Dot3"
                | "Semicolon"
                | "Colon"
                | "ColonEq"
                | "Tilde2"
                | "Backslash"
                | "LAngle"
                | "RAngle"
                | "LParen"
                | "RParen"
                | "LBrack"
                | "RBrack"
                | "LBrace"
                | "RBrace"
        )
    }

    fn visit(value: &Value, decoded: &mut usize) {
        if let Some(object) = value.as_object()
            && let Some(Value::Array(variant)) = object.get("it")
            && let Some(tag) = variant.first().and_then(Value::as_str)
            && is_atom_variant(tag)
        {
            let atom = AtomPhraseCodec::decode(value).expect("decode golden atom phrase");
            assert_eq!(AtomPhraseCodec::encode(&atom), *value);
            *decoded += 1;
        }

        match value {
            Value::Array(items) => {
                for item in items {
                    visit(item, decoded);
                }
            }
            Value::Object(fields) => {
                for item in fields.values() {
                    visit(item, decoded);
                }
            }
            _ => {}
        }
    }

    let fixture = include_bytes!("../../p4spec/test/wire/minimal-value.expected.json");
    let envelope = Envelope::<Value>::from_slice(fixture).expect("decode OCaml value envelope");
    let mut decoded = 0;
    visit(envelope.payload(), &mut decoded);

    assert!(decoded >= 8, "expected atoms in the OCaml value fixture");
}
