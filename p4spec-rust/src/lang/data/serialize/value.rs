//! Native value trees retain arena annotations and opaque JSON extern state

use super::{
    DecodeError, array,
    atom::AtomPhraseCodec,
    boolean, decode_list, encode_list, field, mixfix, num as num_codec, object, on_codec_stack,
    source, string,
    typ::{decode_id, decode_typ_kind, encode_id, encode_typ_kind},
    variant,
};
use crate::{
    lang::data::value::{Value, ValueArena, ValueKind, make},
    util::json::json,
};
use serde_json::json;

pub fn decode(arena: &mut ValueArena, json: &json) -> Result<Value, DecodeError> {
    on_codec_stack(|| decode_value(arena, json))
}

pub fn encode(arena: &ValueArena, value: &Value) -> json {
    on_codec_stack(|| encode_value(arena, value))
}

fn decode_value(arena: &mut ValueArena, json: &json) -> Result<Value, DecodeError> {
    let fields = object(json)?;
    let value_kind = decode_value_kind(arena, field(fields, "it")?)?;
    let typ = decode_typ_kind(field(fields, "typ")?)?;
    let span = source::decode_region(field(fields, "at")?)?;
    Ok(make::new(arena, value_kind, typ.into(), span)?)
}

fn decode_value_kind(arena: &mut ValueArena, json: &json) -> Result<ValueKind, DecodeError> {
    let (tag, fields) = variant(json)?;
    match (tag, fields) {
        ("BoolV", [json]) => Ok(ValueKind::Bool(boolean(json)?)),
        ("NumV", [json]) => Ok(ValueKind::Num(num_codec::decode_num(json)?)),
        ("TextV", [json]) => Ok(ValueKind::Text(string(json)?.to_owned())),
        ("StructV", [json]) => Ok(ValueKind::Struct(decode_list(json, |json| {
            match array(json)? {
                [json_atom, json_value] => Ok((
                    AtomPhraseCodec::decode(json_atom)?,
                    decode_value(arena, json_value)?,
                )),
                _ => Err(DecodeError::Expected("IL value field pair")),
            }
        })?)),
        ("CaseV", [json]) => Ok(ValueKind::Case(mixfix::decode(json, |json| {
            decode_value(arena, json)
        })?)),
        ("TupleV", [json]) => Ok(ValueKind::Tuple(decode_list(json, |json| {
            decode_value(arena, json)
        })?)),
        ("OptV", [json::Null]) => Ok(ValueKind::Opt(None)),
        ("OptV", [json]) => Ok(ValueKind::Opt(Some(decode_value(arena, json)?))),
        ("ListV", [json]) => Ok(ValueKind::List(decode_list(json, |json| {
            decode_value(arena, json)
        })?)),
        ("FuncV", [json]) => Ok(ValueKind::Func(decode_id(json)?)),
        ("ExternV", [json]) => Ok(ValueKind::Extern(json.clone())),
        (
            "BoolV" | "NumV" | "TextV" | "StructV" | "CaseV" | "TupleV" | "OptV" | "ListV"
            | "FuncV" | "ExternV",
            _,
        ) => Err(DecodeError::Expected("valid IL value arity")),
        (unknown, _) => Err(DecodeError::UnknownVariant(unknown.to_owned())),
    }
}

fn encode_value(arena: &ValueArena, value: &Value) -> json {
    json!({
        "it": encode_value_kind(arena, arena.kind(value)),
        "typ": encode_typ_kind(arena.typ(value)),
        "at": source::encode_region(arena.span(value)),
    })
}

fn encode_value_kind(arena: &ValueArena, value_kind: &ValueKind) -> json {
    match value_kind {
        ValueKind::Bool(value) => json!(["BoolV", value]),
        ValueKind::Num(num) => json!(["NumV", num_codec::encode_num(num)]),
        ValueKind::Text(text) => json!(["TextV", text]),
        ValueKind::Struct(fields) => json!([
            "StructV",
            fields
                .iter()
                .map(|(atom, value)| json!([
                    AtomPhraseCodec::encode(atom),
                    encode_value(arena, value),
                ]))
                .collect::<Vec<_>>()
        ]),
        ValueKind::Case(case) => json!([
            "CaseV",
            mixfix::encode(case, |value| encode_value(arena, value)),
        ]),
        ValueKind::Tuple(values) => json!([
            "TupleV",
            encode_list(values, |value| encode_value(arena, value)),
        ]),
        ValueKind::Opt(value) => json!([
            "OptV",
            value.as_ref().map(|value| encode_value(arena, value)),
        ]),
        ValueKind::List(values) => json!([
            "ListV",
            encode_list(values, |value| encode_value(arena, value)),
        ]),
        ValueKind::Func(id) => json!(["FuncV", encode_id(id)]),
        ValueKind::Extern(json) => json!(["ExternV", json]),
    }
}
