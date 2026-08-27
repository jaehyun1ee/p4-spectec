use serde_json::{Value, json};

use crate::lang::common::notation::{atom::Atom, mixfix::AtomPhrase};

use super::{DecodeError, source, string, variant};

pub struct AtomPhraseCodec;

impl AtomPhraseCodec {
    pub fn decode(value: &Value) -> Result<AtomPhrase, DecodeError> {
        source::decode_phrase(value, decode_atom)
    }

    pub fn encode(atom: &AtomPhrase) -> Value {
        source::encode_phrase(atom, encode_atom)
    }
}

fn decode_atom(value: &Value) -> Result<Atom, DecodeError> {
    let (tag, fields) = variant(value)?;
    match (tag, fields) {
        ("Keyword", [identifier]) => Ok(Atom::Keyword(string(identifier)?.to_owned())),
        ("Tag", [identifier]) => Ok(Atom::Tag(string(identifier)?.to_owned())),
        ("Operator", [operator]) => Ok(Atom::Operator(string(operator)?.to_owned())),
        ("Sub", []) => Ok(Atom::Sub),
        ("Sup", []) => Ok(Atom::Sup),
        ("Turnstile", []) => Ok(Atom::Turnstile),
        ("Tilesturn", []) => Ok(Atom::Tilesturn),
        ("Arrow", []) => Ok(Atom::Arrow),
        ("ArrowSub", []) => Ok(Atom::ArrowSub),
        ("DoubleArrowSub", []) => Ok(Atom::DoubleArrowSub),
        ("DoubleArrowLong", []) => Ok(Atom::DoubleArrowLong),
        ("SqArrow", []) => Ok(Atom::SqArrow),
        ("SqArrowStar", []) => Ok(Atom::SqArrowStar),
        ("Dot", []) => Ok(Atom::Dot),
        ("Dot2", []) => Ok(Atom::Dot2),
        ("Dot3", []) => Ok(Atom::Dot3),
        ("Semicolon", []) => Ok(Atom::Semicolon),
        ("Colon", []) => Ok(Atom::Colon),
        ("ColonEq", []) => Ok(Atom::ColonEq),
        ("Tilde2", []) => Ok(Atom::Tilde2),
        ("Backslash", []) => Ok(Atom::Backslash),
        ("LAngle", []) => Ok(Atom::LAngle),
        ("RAngle", []) => Ok(Atom::RAngle),
        ("LParen", []) => Ok(Atom::LParen),
        ("RParen", []) => Ok(Atom::RParen),
        ("LBrack", []) => Ok(Atom::LBrack),
        ("RBrack", []) => Ok(Atom::RBrack),
        ("LBrace", []) => Ok(Atom::LBrace),
        ("RBrace", []) => Ok(Atom::RBrace),
        (known, _) if is_known_variant(known) => Err(DecodeError::Expected("valid atom arity")),
        (unknown, _) => Err(DecodeError::UnknownVariant(unknown.to_owned())),
    }
}

fn is_known_variant(tag: &str) -> bool {
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

fn encode_atom(atom: &Atom) -> Value {
    match atom {
        Atom::Keyword(identifier) => json!(["Keyword", identifier]),
        Atom::Tag(identifier) => json!(["Tag", identifier]),
        Atom::Operator(operator) => json!(["Operator", operator]),
        Atom::Sub => json!(["Sub"]),
        Atom::Sup => json!(["Sup"]),
        Atom::Turnstile => json!(["Turnstile"]),
        Atom::Tilesturn => json!(["Tilesturn"]),
        Atom::Arrow => json!(["Arrow"]),
        Atom::ArrowSub => json!(["ArrowSub"]),
        Atom::DoubleArrowSub => json!(["DoubleArrowSub"]),
        Atom::DoubleArrowLong => json!(["DoubleArrowLong"]),
        Atom::SqArrow => json!(["SqArrow"]),
        Atom::SqArrowStar => json!(["SqArrowStar"]),
        Atom::Dot => json!(["Dot"]),
        Atom::Dot2 => json!(["Dot2"]),
        Atom::Dot3 => json!(["Dot3"]),
        Atom::Semicolon => json!(["Semicolon"]),
        Atom::Colon => json!(["Colon"]),
        Atom::ColonEq => json!(["ColonEq"]),
        Atom::Tilde2 => json!(["Tilde2"]),
        Atom::Backslash => json!(["Backslash"]),
        Atom::LAngle => json!(["LAngle"]),
        Atom::RAngle => json!(["RAngle"]),
        Atom::LParen => json!(["LParen"]),
        Atom::RParen => json!(["RParen"]),
        Atom::LBrack => json!(["LBrack"]),
        Atom::RBrack => json!(["RBrack"]),
        Atom::LBrace => json!(["LBrace"]),
        Atom::RBrace => json!(["RBrace"]),
    }
}
