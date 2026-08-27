/// OCaml-compatible JSON codecs for IL data
use std::{cell::Cell, collections::HashSet};

use serde_json::{Map, Number, Value, json};
use thiserror::Error;

use crate::lang::{
    common::{notation::mixfix::Mixfix, noted::Noted, source::Spanned},
    il::ast::{self, *},
    xl::{bool, num},
};
use crate::yojson::ExternalData;

use super::{
    super::{
        DecodeError, EncodeError, array, boolean, decode_list, decode_option, encode_list,
        encode_option, field, integer, object, on_codec_stack, string, variant,
    },
    el, xl,
};
use crate::wire::VALUE_SCHEMA;
use crate::wire::ocaml::{atom::AtomPhraseCodec, mixfix, source, yojson};

/// Codec for complete IL specifications
pub struct SpecCodec;

impl SpecCodec {
    pub fn decode(value: &Value) -> Result<ast::Spec, DecodeError> {
        on_codec_stack(|| decode_list(value, decode_def))
    }

    pub fn encode(spec: &ast::Spec) -> Result<Value, EncodeError> {
        on_codec_stack(|| Ok(encode_list(spec, encode_def)))
    }
}

pub struct ValueCodec;

/// Standard-JSON convenience codec for IL values
///
/// Use [`ValueEnvelopeCodec`] for lossless OCaml `Yojson.Safe` transport
impl ValueCodec {
    pub fn decode(value: &Value) -> Result<ast::Value, DecodeError> {
        on_codec_stack(|| decode_value(value))
    }

    pub fn encode(value: &ast::Value) -> Result<Value, EncodeError> {
        on_codec_stack(|| ValueEncoder::default().encode_value(value))
    }
}

/// Lossless codec for the versioned OCaml IL value envelope
pub struct ValueEnvelopeCodec;

impl ValueEnvelopeCodec {
    pub fn decode(input: &[u8]) -> Result<ast::Value, ValueEnvelopeDecodeError> {
        on_codec_stack(|| {
            let envelope = yojson::Value::from_slice(input)?;
            let fields = yojson_assoc(&envelope)?;
            let schema = yojson_string(yojson_field(fields, "schema")?)?;
            let kind = yojson_string(yojson_field(fields, "kind")?)?;

            if schema != VALUE_SCHEMA {
                return Err(ValueEnvelopeDecodeError::UnknownSchema(schema.to_owned()));
            }
            if kind != "value" {
                return Err(ValueEnvelopeDecodeError::SchemaKindMismatch(
                    kind.to_owned(),
                ));
            }

            decode_yojson_value(yojson_field(fields, "payload")?).map_err(Into::into)
        })
    }

    pub fn encode(value: &ast::Value) -> Result<Vec<u8>, ValueEnvelopeEncodeError> {
        on_codec_stack(|| {
            let encoder = ValueEncoder::default();
            let envelope = yojson::Value::Assoc(vec![
                (
                    "schema".to_owned(),
                    yojson::Value::String(VALUE_SCHEMA.to_owned()),
                ),
                ("kind".to_owned(), yojson::Value::String("value".to_owned())),
                ("payload".to_owned(), encoder.encode_yojson_value(value)),
            ]);
            envelope.to_vec().map_err(Into::into)
        })
    }
}

#[derive(Debug, Error)]
pub enum ValueEnvelopeDecodeError {
    #[error("invalid Yojson value envelope: {0}")]
    Parse(#[from] yojson::ParseError),

    #[error("invalid OCaml IL value: {0}")]
    Decode(#[from] DecodeError),

    #[error("unknown wire schema `{0}`")]
    UnknownSchema(String),

    #[error("schema `p4spectec.value.v1` requires kind `value`, but found `{0}`")]
    SchemaKindMismatch(String),
}

#[derive(Debug, Error)]
pub enum ValueEnvelopeEncodeError {
    #[error("cannot write Yojson value envelope: {0}")]
    Write(#[from] yojson::WriteError),
}

pub(super) fn decode_id(value: &Value) -> Result<ast::Id, DecodeError> {
    source::decode_phrase(value, |value| Ok(string(value)?.to_owned()))
}

pub(super) fn encode_id(id: &ast::Id) -> Value {
    source::encode_phrase(id, |id| json!(id))
}

pub(super) fn decode_iter(value: &Value) -> Result<Iter, DecodeError> {
    let (tag, fields) = variant(value)?;
    match (tag, fields) {
        ("Opt", []) => Ok(Iter::Opt),
        ("List", []) => Ok(Iter::List),
        ("Opt" | "List", _) => Err(DecodeError::Expected("valid IL iterator arity")),
        (unknown, _) => Err(DecodeError::UnknownVariant(unknown.to_owned())),
    }
}

pub(super) fn encode_iter(iter: Iter) -> Value {
    match iter {
        Iter::Opt => json!(["Opt"]),
        Iter::List => json!(["List"]),
    }
}

pub(super) fn decode_var(value: &Value) -> Result<ast::Var, DecodeError> {
    match array(value)? {
        [id, typ, iters] => Ok(ast::Var {
            id: decode_id(id)?,
            typ: decode_typ(typ)?,
            iters: decode_list(iters, decode_iter)?,
        }),
        _ => Err(DecodeError::Expected("IL variable triple")),
    }
}

pub(super) fn encode_var(variable: &ast::Var) -> Value {
    json!([
        encode_id(&variable.id),
        encode_typ(&variable.typ),
        encode_list(&variable.iters, |iter| encode_iter(*iter))
    ])
}

pub(super) fn decode_typ(value: &Value) -> Result<ast::Typ, DecodeError> {
    source::decode_phrase(value, decode_typ_kind)
}

pub(super) fn encode_typ(typ: &ast::Typ) -> Value {
    source::encode_phrase(typ, encode_typ_kind)
}

pub(super) fn decode_targ(value: &Value) -> Result<ast::Targ, DecodeError> {
    source::decode_phrase(value, decode_typ_kind)
}

pub(super) fn encode_targ(targ: &ast::Targ) -> Value {
    source::encode_phrase(targ, encode_typ_kind)
}

pub(super) fn decode_tparam(value: &Value) -> Result<ast::TParam, DecodeError> {
    source::decode_phrase(value, |value| Ok(string(value)?.to_owned()))
}

pub(super) fn encode_tparam(tparam: &ast::TParam) -> Value {
    source::encode_phrase(tparam, |tparam| json!(tparam))
}

pub(super) fn decode_typ_kind(value: &Value) -> Result<TypKind, DecodeError> {
    let (tag, fields) = variant(value)?;
    match (tag, fields) {
        ("BoolT", []) => Ok(TypKind::Bool),
        ("NumT", [typ]) => Ok(TypKind::Num(xl::decode_num_typ(typ)?)),
        ("TextT", []) => Ok(TypKind::Text),
        ("VarT", [id, targs]) => Ok(TypKind::Var(
            decode_id(id)?,
            decode_list(targs, decode_targ)?,
        )),
        ("TupleT", [types]) => Ok(TypKind::Tuple(decode_list(types, decode_typ)?)),
        ("IterT", [typ, iter]) => Ok(TypKind::Iter(
            Box::new(decode_typ(typ)?),
            decode_iter(iter)?,
        )),
        ("FuncT", [tparams, params, result]) => Ok(TypKind::Func(
            decode_list(tparams, decode_tparam)?,
            decode_list(params, decode_typ)?,
            Box::new(decode_typ(result)?),
        )),
        ("BoolT" | "NumT" | "TextT" | "VarT" | "TupleT" | "IterT" | "FuncT", _) => {
            Err(DecodeError::Expected("valid IL type arity"))
        }
        (unknown, _) => Err(DecodeError::UnknownVariant(unknown.to_owned())),
    }
}

pub(super) fn encode_typ_kind(typ: &TypKind) -> Value {
    match typ {
        TypKind::Bool => json!(["BoolT"]),
        TypKind::Num(typ) => json!(["NumT", xl::encode_num_typ(*typ)]),
        TypKind::Text => json!(["TextT"]),
        TypKind::Var(id, targs) => {
            json!(["VarT", encode_id(id), encode_list(targs, encode_targ)])
        }
        TypKind::Tuple(types) => json!(["TupleT", encode_list(types, encode_typ)]),
        TypKind::Iter(typ, iter) => json!(["IterT", encode_typ(typ), encode_iter(*iter)]),
        TypKind::Func(tparams, params, result) => json!([
            "FuncT",
            encode_list(tparams, encode_tparam),
            encode_list(params, encode_typ),
            encode_typ(result)
        ]),
    }
}

pub(super) fn decode_not_typ(value: &Value) -> Result<ast::NotTyp, DecodeError> {
    source::decode_phrase(value, |value| mixfix::decode(value, decode_typ))
}

pub(super) fn encode_not_typ(typ: &ast::NotTyp) -> Value {
    source::encode_phrase(typ, |typ| mixfix::encode(typ, encode_typ))
}

fn decode_typ_origin(value: &Value) -> Result<ast::TypOrigin, DecodeError> {
    source::decode_phrase(value, |value| match array(value)? {
        [id, targs] => Ok((decode_id(id)?, decode_list(targs, decode_targ)?)),
        _ => Err(DecodeError::Expected("IL type origin pair")),
    })
}

fn encode_typ_origin(origin: &ast::TypOrigin) -> Value {
    source::encode_phrase(origin, |(id, targs)| {
        json!([encode_id(id), encode_list(targs, encode_targ)])
    })
}

pub(super) fn decode_def_typ(value: &Value) -> Result<ast::DefTyp, DecodeError> {
    source::decode_phrase(value, |value| {
        let (tag, fields) = variant(value)?;
        match (tag, fields) {
            ("PlainT", [typ]) => Ok(DefTypKind::Plain(decode_typ(typ)?)),
            ("StructT", [fields]) => Ok(DefTypKind::Struct(decode_list(
                fields,
                |field| match array(field)? {
                    [atom, typ] => Ok((AtomPhraseCodec::decode(atom)?, decode_typ(typ)?)),
                    _ => Err(DecodeError::Expected("IL type field pair")),
                },
            )?)),
            ("VariantT", [cases]) => Ok(DefTypKind::Variant(decode_list(
                cases,
                |case| match array(case)? {
                    [not_typ, typ_origin, hints] => Ok((
                        decode_not_typ(not_typ)?,
                        decode_typ_origin(typ_origin)?,
                        decode_list(hints, el::decode_hint)?,
                    )),
                    _ => Err(DecodeError::Expected("IL type case triple")),
                },
            )?)),
            ("PlainT" | "StructT" | "VariantT", _) => {
                Err(DecodeError::Expected("valid IL defined type arity"))
            }
            (unknown, _) => Err(DecodeError::UnknownVariant(unknown.to_owned())),
        }
    })
}

pub(super) fn encode_def_typ(typ: &ast::DefTyp) -> Value {
    source::encode_phrase(typ, |typ| match typ {
        DefTypKind::Plain(typ) => json!(["PlainT", encode_typ(typ)]),
        DefTypKind::Struct(fields) => json!([
            "StructT",
            fields
                .iter()
                .map(|(atom, typ)| json!([AtomPhraseCodec::encode(atom), encode_typ(typ)]))
                .collect::<Vec<_>>()
        ]),
        DefTypKind::Variant(cases) => json!([
            "VariantT",
            cases
                .iter()
                .map(|(not_typ, typ_origin, hints)| json!([
                    encode_not_typ(not_typ),
                    encode_typ_origin(typ_origin),
                    encode_list(hints, el::encode_hint)
                ]))
                .collect::<Vec<_>>()
        ]),
    })
}

fn decode_vnote(value: &Value) -> Result<TypKind, DecodeError> {
    let object = object(value)?;
    integer(field(object, "vid")?)?;
    let typ = decode_typ_kind(field(object, "typ")?)?;
    integer(field(object, "vhash")?)?;
    Ok(typ)
}

#[derive(Default)]
struct ValueEncoder {
    next_vid: Cell<i64>,
}

impl ValueEncoder {
    fn encode_vnote(&self, typ: &TypKind) -> Value {
        let vid = self.next_vid.get();
        self.next_vid.set(vid + 1);

        json!({
            "vid": vid,
            "typ": encode_typ_kind(typ),
            // A constant hash preserves equality correctness but disables fast rejection
            "vhash": 0,
        })
    }
}

fn decode_external(value: &Value) -> ExternalData {
    match value {
        Value::Null => ExternalData::Null,
        Value::Bool(value) => ExternalData::Bool(*value),
        Value::Number(number) => {
            if let Some(integer) = number.as_i64() {
                ExternalData::Int(integer)
            } else if number.is_u64() {
                ExternalData::Intlit(number.to_string())
            } else if let Some(float) = number.as_f64() {
                ExternalData::Float(float)
            } else {
                ExternalData::Intlit(number.to_string())
            }
        }
        Value::String(value) => ExternalData::String(value.clone()),
        Value::Array(values) => ExternalData::List(values.iter().map(decode_external).collect()),
        Value::Object(fields) => ExternalData::Assoc(
            fields
                .iter()
                .map(|(name, value)| (name.clone(), decode_external(value)))
                .collect(),
        ),
    }
}

fn unsupported_external(reason: impl Into<String>) -> EncodeError {
    EncodeError::UnsupportedExternalData(reason.into())
}

fn encode_external(value: &ExternalData) -> Result<Value, EncodeError> {
    match value {
        ExternalData::Null => Ok(Value::Null),
        ExternalData::Bool(value) => Ok(Value::Bool(*value)),
        ExternalData::Int(value) => Ok(Value::Number(Number::from(*value))),
        ExternalData::Intlit(value) => match serde_json::from_str(value) {
            Ok(Value::Number(number))
                if !value.contains(['.', 'e', 'E']) && number.to_string() == *value =>
            {
                Ok(Value::Number(number))
            }
            _ => Err(unsupported_external(format!(
                "integer literal `{value}` is not representable by serde_json"
            ))),
        },
        ExternalData::Float(value) => Number::from_f64(*value)
            .map(Value::Number)
            .ok_or_else(|| unsupported_external("non-finite float")),
        ExternalData::String(value) => Ok(Value::String(value.clone())),
        ExternalData::Assoc(fields) => {
            let mut names = HashSet::with_capacity(fields.len());
            let mut object = Map::new();
            for (name, value) in fields {
                if !names.insert(name) {
                    return Err(unsupported_external(format!(
                        "duplicate object field `{name}`"
                    )));
                }
                object.insert(name.clone(), encode_external(value)?);
            }
            Ok(Value::Object(object))
        }
        ExternalData::List(values) => Ok(Value::Array(
            values
                .iter()
                .map(encode_external)
                .collect::<Result<_, _>>()?,
        )),
        ExternalData::Tuple(_) => Err(unsupported_external("non-standard JSON tuple")),
        ExternalData::Variant(_, _) => Err(unsupported_external("non-standard JSON variant")),
    }
}

fn yojson_assoc(value: &yojson::Value) -> Result<&[(String, yojson::Value)], DecodeError> {
    match value {
        yojson::Value::Assoc(fields) => Ok(fields),
        _ => Err(DecodeError::Expected("Yojson association")),
    }
}

fn yojson_list(value: &yojson::Value) -> Result<&[yojson::Value], DecodeError> {
    match value {
        yojson::Value::List(values) => Ok(values),
        _ => Err(DecodeError::Expected("Yojson list")),
    }
}

fn yojson_string(value: &yojson::Value) -> Result<&str, DecodeError> {
    match value {
        yojson::Value::String(value) => Ok(value),
        _ => Err(DecodeError::Expected("Yojson string")),
    }
}

fn yojson_boolean(value: &yojson::Value) -> Result<bool, DecodeError> {
    match value {
        yojson::Value::Bool(value) => Ok(*value),
        _ => Err(DecodeError::Expected("Yojson boolean")),
    }
}

fn yojson_field<'a>(
    fields: &'a [(String, yojson::Value)],
    name: &'static str,
) -> Result<&'a yojson::Value, DecodeError> {
    let mut matches = fields.iter().filter(|(field, _)| field == name);
    let value = matches
        .next()
        .map(|(_, value)| value)
        .ok_or(DecodeError::MissingField(name))?;
    if matches.next().is_some() {
        return Err(DecodeError::Expected(
            "OCaml record without duplicate fields",
        ));
    }
    Ok(value)
}

fn yojson_variant(value: &yojson::Value) -> Result<(&str, &[yojson::Value]), DecodeError> {
    let values = yojson_list(value)?;
    let (tag, fields) = values
        .split_first()
        .ok_or(DecodeError::Expected("non-empty Yojson variant list"))?;
    Ok((yojson_string(tag)?, fields))
}

fn standard_json(value: &yojson::Value) -> Result<Value, DecodeError> {
    yojson::to_serde_json(value).map_err(DecodeError::Expected)
}

fn decode_yojson_external(value: &yojson::Value) -> ExternalData {
    match value {
        yojson::Value::Null => ExternalData::Null,
        yojson::Value::Bool(value) => ExternalData::Bool(*value),
        yojson::Value::Int(value) => ExternalData::Int(*value),
        yojson::Value::Intlit(value) => ExternalData::Intlit(value.clone()),
        yojson::Value::Float(value) => ExternalData::Float(*value),
        yojson::Value::String(value) => ExternalData::String(value.clone()),
        yojson::Value::Assoc(fields) => ExternalData::Assoc(
            fields
                .iter()
                .map(|(name, value)| (name.clone(), decode_yojson_external(value)))
                .collect(),
        ),
        yojson::Value::List(values) => {
            ExternalData::List(values.iter().map(decode_yojson_external).collect())
        }
        yojson::Value::Tuple(values) => {
            ExternalData::Tuple(values.iter().map(decode_yojson_external).collect())
        }
        yojson::Value::Variant(name, value) => ExternalData::Variant(
            name.clone(),
            value.as_deref().map(decode_yojson_external).map(Box::new),
        ),
    }
}

fn encode_yojson_external(value: &ExternalData) -> yojson::Value {
    match value {
        ExternalData::Null => yojson::Value::Null,
        ExternalData::Bool(value) => yojson::Value::Bool(*value),
        ExternalData::Int(value) => yojson::Value::Int(*value),
        ExternalData::Intlit(value) => yojson::Value::Intlit(value.clone()),
        ExternalData::Float(value) => yojson::Value::Float(*value),
        ExternalData::String(value) => yojson::Value::String(value.clone()),
        ExternalData::Assoc(fields) => yojson::Value::Assoc(
            fields
                .iter()
                .map(|(name, value)| (name.clone(), encode_yojson_external(value)))
                .collect(),
        ),
        ExternalData::List(values) => {
            yojson::Value::List(values.iter().map(encode_yojson_external).collect())
        }
        ExternalData::Tuple(values) => {
            yojson::Value::Tuple(values.iter().map(encode_yojson_external).collect())
        }
        ExternalData::Variant(name, value) => yojson::Value::Variant(
            name.clone(),
            value.as_deref().map(encode_yojson_external).map(Box::new),
        ),
    }
}

fn decode_yojson_mixfix(value: &yojson::Value) -> Result<ast::ValueCase, DecodeError> {
    let (tag, fields) = yojson_variant(value)?;
    match (tag, fields) {
        ("Arg", [arg]) => Ok(Mixfix::Arg(decode_yojson_value(arg)?)),
        ("Atom", [atom]) => Ok(Mixfix::Atom(AtomPhraseCodec::decode(&standard_json(
            atom,
        )?)?)),
        ("Brack", [left, body, right]) => Ok(Mixfix::Brack(
            AtomPhraseCodec::decode(&standard_json(left)?)?,
            Box::new(decode_yojson_mixfix(body)?),
            AtomPhraseCodec::decode(&standard_json(right)?)?,
        )),
        ("Infix", [left, atom, right]) => Ok(Mixfix::Infix(
            Box::new(decode_yojson_mixfix(left)?),
            AtomPhraseCodec::decode(&standard_json(atom)?)?,
            Box::new(decode_yojson_mixfix(right)?),
        )),
        ("Seq", [items]) => Ok(Mixfix::Seq(
            yojson_list(items)?
                .iter()
                .map(decode_yojson_mixfix)
                .collect::<Result<_, _>>()?,
        )),
        ("Arg" | "Atom" | "Brack" | "Infix" | "Seq", _) => {
            Err(DecodeError::Expected("valid Yojson mixfix arity"))
        }
        (unknown, _) => Err(DecodeError::UnknownVariant(unknown.to_owned())),
    }
}

fn decode_yojson_value(value: &yojson::Value) -> Result<ast::Value, DecodeError> {
    let fields = yojson_assoc(value)?;
    Ok(Spanned::new(
        Noted::new(
            decode_yojson_value_kind(yojson_field(fields, "it")?)?,
            decode_vnote(&standard_json(yojson_field(fields, "note")?)?)?,
        ),
        source::decode_region(&standard_json(yojson_field(fields, "at")?)?)?,
    ))
}

fn decode_yojson_value_kind(value: &yojson::Value) -> Result<ValueKind, DecodeError> {
    let (tag, fields) = yojson_variant(value)?;
    match (tag, fields) {
        ("BoolV", [value]) => Ok(ValueKind::Bool(yojson_boolean(value)?)),
        ("NumV", [num]) => Ok(ValueKind::Num(xl::decode_num(&standard_json(num)?)?)),
        ("TextV", [text]) => Ok(ValueKind::Text(yojson_string(text)?.to_owned())),
        ("StructV", [fields]) => Ok(ValueKind::Struct(
            yojson_list(fields)?
                .iter()
                .map(|field| match yojson_list(field)? {
                    [atom, value] => Ok((
                        AtomPhraseCodec::decode(&standard_json(atom)?)?,
                        decode_yojson_value(value)?,
                    )),
                    _ => Err(DecodeError::Expected("IL value field pair")),
                })
                .collect::<Result<_, _>>()?,
        )),
        ("CaseV", [case]) => Ok(ValueKind::Case(Box::new(decode_yojson_mixfix(case)?))),
        ("TupleV", [values]) => Ok(ValueKind::Tuple(
            yojson_list(values)?
                .iter()
                .map(decode_yojson_value)
                .collect::<Result<_, _>>()?,
        )),
        ("OptV", [yojson::Value::Null]) => Ok(ValueKind::Opt(None)),
        ("OptV", [value]) => Ok(ValueKind::Opt(Some(Box::new(decode_yojson_value(value)?)))),
        ("ListV", [values]) => Ok(ValueKind::List(
            yojson_list(values)?
                .iter()
                .map(decode_yojson_value)
                .collect::<Result<_, _>>()?,
        )),
        ("FuncV", [id]) => Ok(ValueKind::Func(decode_id(&standard_json(id)?)?)),
        ("ExternV", [value]) => Ok(ValueKind::Extern(decode_yojson_external(value))),
        (
            "BoolV" | "NumV" | "TextV" | "StructV" | "CaseV" | "TupleV" | "OptV" | "ListV"
            | "FuncV" | "ExternV",
            _,
        ) => Err(DecodeError::Expected("valid IL value arity")),
        (unknown, _) => Err(DecodeError::UnknownVariant(unknown.to_owned())),
    }
}

fn decode_value(value: &Value) -> Result<ast::Value, DecodeError> {
    let (kind, typ, span) = source::decode_annotated(value, decode_value_kind, decode_vnote)?;
    Ok(Spanned::new(Noted::new(kind, typ), span))
}

fn decode_value_kind(value: &Value) -> Result<ValueKind, DecodeError> {
    let (tag, fields) = variant(value)?;
    match (tag, fields) {
        ("BoolV", [value]) => Ok(ValueKind::Bool(boolean(value)?)),
        ("NumV", [num]) => Ok(ValueKind::Num(xl::decode_num(num)?)),
        ("TextV", [text]) => Ok(ValueKind::Text(string(text)?.to_owned())),
        ("StructV", [fields]) => Ok(ValueKind::Struct(decode_list(
            fields,
            |field| match array(field)? {
                [atom, value] => Ok((AtomPhraseCodec::decode(atom)?, decode_value(value)?)),
                _ => Err(DecodeError::Expected("IL value field pair")),
            },
        )?)),
        ("CaseV", [case]) => Ok(ValueKind::Case(Box::new(mixfix::decode(
            case,
            decode_value,
        )?))),
        ("TupleV", [values]) => Ok(ValueKind::Tuple(decode_list(values, decode_value)?)),
        ("OptV", [value]) => Ok(ValueKind::Opt(
            decode_option(value, decode_value)?.map(Box::new),
        )),
        ("ListV", [values]) => Ok(ValueKind::List(decode_list(values, decode_value)?)),
        ("FuncV", [id]) => Ok(ValueKind::Func(decode_id(id)?)),
        ("ExternV", [value]) => Ok(ValueKind::Extern(decode_external(value))),
        (
            "BoolV" | "NumV" | "TextV" | "StructV" | "CaseV" | "TupleV" | "OptV" | "ListV"
            | "FuncV" | "ExternV",
            _,
        ) => Err(DecodeError::Expected("valid IL value arity")),
        (unknown, _) => Err(DecodeError::UnknownVariant(unknown.to_owned())),
    }
}

impl ValueEncoder {
    fn encode_yojson_mixfix(&self, value: &ast::ValueCase) -> yojson::Value {
        match value {
            Mixfix::Arg(value) => yojson::Value::List(vec![
                yojson::Value::String("Arg".to_owned()),
                self.encode_yojson_value(value),
            ]),
            Mixfix::Atom(atom) => yojson::Value::List(vec![
                yojson::Value::String("Atom".to_owned()),
                yojson::from_serde_json(&AtomPhraseCodec::encode(atom)),
            ]),
            Mixfix::Brack(left, body, right) => yojson::Value::List(vec![
                yojson::Value::String("Brack".to_owned()),
                yojson::from_serde_json(&AtomPhraseCodec::encode(left)),
                self.encode_yojson_mixfix(body),
                yojson::from_serde_json(&AtomPhraseCodec::encode(right)),
            ]),
            Mixfix::Infix(left, atom, right) => yojson::Value::List(vec![
                yojson::Value::String("Infix".to_owned()),
                self.encode_yojson_mixfix(left),
                yojson::from_serde_json(&AtomPhraseCodec::encode(atom)),
                self.encode_yojson_mixfix(right),
            ]),
            Mixfix::Seq(items) => yojson::Value::List(vec![
                yojson::Value::String("Seq".to_owned()),
                yojson::Value::List(
                    items
                        .iter()
                        .map(|item| self.encode_yojson_mixfix(item))
                        .collect(),
                ),
            ]),
        }
    }

    fn encode_yojson_value(&self, value: &ast::Value) -> yojson::Value {
        let kind = self.encode_yojson_value_kind(&value.node.kind);
        yojson::Value::Assoc(vec![
            ("it".to_owned(), kind),
            (
                "note".to_owned(),
                yojson::from_serde_json(&self.encode_vnote(&value.node.note)),
            ),
            (
                "at".to_owned(),
                yojson::from_serde_json(&source::encode_region(&value.span)),
            ),
        ])
    }

    fn encode_yojson_value_kind(&self, value: &ValueKind) -> yojson::Value {
        let fields = match value {
            ValueKind::Bool(value) => vec![
                yojson::Value::String("BoolV".to_owned()),
                yojson::Value::Bool(*value),
            ],
            ValueKind::Num(num) => vec![
                yojson::Value::String("NumV".to_owned()),
                yojson::from_serde_json(&xl::encode_num(num)),
            ],
            ValueKind::Text(text) => vec![
                yojson::Value::String("TextV".to_owned()),
                yojson::Value::String(text.clone()),
            ],
            ValueKind::Struct(fields) => vec![
                yojson::Value::String("StructV".to_owned()),
                yojson::Value::List(
                    fields
                        .iter()
                        .map(|(atom, value)| {
                            yojson::Value::List(vec![
                                yojson::from_serde_json(&AtomPhraseCodec::encode(atom)),
                                self.encode_yojson_value(value),
                            ])
                        })
                        .collect(),
                ),
            ],
            ValueKind::Case(case) => vec![
                yojson::Value::String("CaseV".to_owned()),
                self.encode_yojson_mixfix(case),
            ],
            ValueKind::Tuple(values) => vec![
                yojson::Value::String("TupleV".to_owned()),
                yojson::Value::List(
                    values
                        .iter()
                        .map(|value| self.encode_yojson_value(value))
                        .collect(),
                ),
            ],
            ValueKind::Opt(value) => vec![
                yojson::Value::String("OptV".to_owned()),
                value
                    .as_deref()
                    .map(|value| self.encode_yojson_value(value))
                    .unwrap_or(yojson::Value::Null),
            ],
            ValueKind::List(values) => vec![
                yojson::Value::String("ListV".to_owned()),
                yojson::Value::List(
                    values
                        .iter()
                        .map(|value| self.encode_yojson_value(value))
                        .collect(),
                ),
            ],
            ValueKind::Func(id) => vec![
                yojson::Value::String("FuncV".to_owned()),
                yojson::from_serde_json(&encode_id(id)),
            ],
            ValueKind::Extern(value) => vec![
                yojson::Value::String("ExternV".to_owned()),
                encode_yojson_external(value),
            ],
        };
        yojson::Value::List(fields)
    }

    fn encode_value(&self, value: &ast::Value) -> Result<Value, EncodeError> {
        let kind = self.encode_value_kind(&value.node.kind)?;
        Ok(json!({
            "it": kind,
            "note": self.encode_vnote(&value.node.note),
            "at": source::encode_region(&value.span),
        }))
    }

    fn encode_value_kind(&self, value: &ValueKind) -> Result<Value, EncodeError> {
        Ok(match value {
            ValueKind::Bool(value) => json!(["BoolV", value]),
            ValueKind::Num(num) => json!(["NumV", xl::encode_num(num)]),
            ValueKind::Text(text) => json!(["TextV", text]),
            ValueKind::Struct(fields) => json!([
                "StructV",
                fields
                    .iter()
                    .map(|(atom, value)| Ok(json!([
                        AtomPhraseCodec::encode(atom),
                        self.encode_value(value)?
                    ])))
                    .collect::<Result<Vec<_>, EncodeError>>()?
            ]),
            ValueKind::Case(case) => {
                json!([
                    "CaseV",
                    mixfix::try_encode(case, |value| self.encode_value(value))?
                ])
            }
            ValueKind::Tuple(values) => json!([
                "TupleV",
                values
                    .iter()
                    .map(|value| self.encode_value(value))
                    .collect::<Result<Vec<_>, _>>()?
            ]),
            ValueKind::Opt(value) => json!([
                "OptV",
                match value {
                    Some(value) => self.encode_value(value)?,
                    None => Value::Null,
                }
            ]),
            ValueKind::List(values) => json!([
                "ListV",
                values
                    .iter()
                    .map(|value| self.encode_value(value))
                    .collect::<Result<Vec<_>, _>>()?
            ]),
            ValueKind::Func(id) => json!(["FuncV", encode_id(id)]),
            ValueKind::Extern(value) => json!(["ExternV", encode_external(value)?]),
        })
    }
}

pub(super) fn decode_un_op(value: &Value) -> Result<UnOp, DecodeError> {
    let (tag, fields) = variant(value)?;
    match (tag, fields) {
        ("NotOp", []) => Ok(UnOp::Bool(bool::UnOp::Not)),
        ("PlusOp", []) => Ok(UnOp::Num(num::UnOp::Plus)),
        ("MinusOp", []) => Ok(UnOp::Num(num::UnOp::Minus)),
        ("NotOp" | "PlusOp" | "MinusOp", _) => {
            Err(DecodeError::Expected("valid IL unary operator arity"))
        }
        (unknown, _) => Err(DecodeError::UnknownVariant(unknown.to_owned())),
    }
}

pub(super) fn encode_un_op(op: UnOp) -> Value {
    match op {
        UnOp::Bool(bool::UnOp::Not) => json!(["NotOp"]),
        UnOp::Num(num::UnOp::Plus) => json!(["PlusOp"]),
        UnOp::Num(num::UnOp::Minus) => json!(["MinusOp"]),
    }
}

pub(super) fn decode_bin_op(value: &Value) -> Result<BinOp, DecodeError> {
    let (tag, fields) = variant(value)?;
    match (tag, fields) {
        ("AndOp", []) => Ok(BinOp::Bool(bool::BinOp::And)),
        ("OrOp", []) => Ok(BinOp::Bool(bool::BinOp::Or)),
        ("ImplOp", []) => Ok(BinOp::Bool(bool::BinOp::Impl)),
        ("EquivOp", []) => Ok(BinOp::Bool(bool::BinOp::Equiv)),
        ("AddOp", []) => Ok(BinOp::Num(num::BinOp::Add)),
        ("SubOp", []) => Ok(BinOp::Num(num::BinOp::Sub)),
        ("MulOp", []) => Ok(BinOp::Num(num::BinOp::Mul)),
        ("DivOp", []) => Ok(BinOp::Num(num::BinOp::Div)),
        ("ModOp", []) => Ok(BinOp::Num(num::BinOp::Mod)),
        ("PowOp", []) => Ok(BinOp::Num(num::BinOp::Pow)),
        (
            "AndOp" | "OrOp" | "ImplOp" | "EquivOp" | "AddOp" | "SubOp" | "MulOp" | "DivOp"
            | "ModOp" | "PowOp",
            _,
        ) => Err(DecodeError::Expected("valid IL binary operator arity")),
        (unknown, _) => Err(DecodeError::UnknownVariant(unknown.to_owned())),
    }
}

pub(super) fn encode_bin_op(op: BinOp) -> Value {
    match op {
        BinOp::Bool(bool::BinOp::And) => json!(["AndOp"]),
        BinOp::Bool(bool::BinOp::Or) => json!(["OrOp"]),
        BinOp::Bool(bool::BinOp::Impl) => json!(["ImplOp"]),
        BinOp::Bool(bool::BinOp::Equiv) => json!(["EquivOp"]),
        BinOp::Num(num::BinOp::Add) => json!(["AddOp"]),
        BinOp::Num(num::BinOp::Sub) => json!(["SubOp"]),
        BinOp::Num(num::BinOp::Mul) => json!(["MulOp"]),
        BinOp::Num(num::BinOp::Div) => json!(["DivOp"]),
        BinOp::Num(num::BinOp::Mod) => json!(["ModOp"]),
        BinOp::Num(num::BinOp::Pow) => json!(["PowOp"]),
    }
}

pub(super) fn decode_cmp_op(value: &Value) -> Result<CmpOp, DecodeError> {
    let (tag, fields) = variant(value)?;
    match (tag, fields) {
        ("EqOp", []) => Ok(CmpOp::Bool(bool::CmpOp::Eq)),
        ("NeOp", []) => Ok(CmpOp::Bool(bool::CmpOp::Ne)),
        ("LtOp", []) => Ok(CmpOp::Num(num::CmpOp::Lt)),
        ("GtOp", []) => Ok(CmpOp::Num(num::CmpOp::Gt)),
        ("LeOp", []) => Ok(CmpOp::Num(num::CmpOp::Le)),
        ("GeOp", []) => Ok(CmpOp::Num(num::CmpOp::Ge)),
        ("EqOp" | "NeOp" | "LtOp" | "GtOp" | "LeOp" | "GeOp", _) => {
            Err(DecodeError::Expected("valid IL comparison operator arity"))
        }
        (unknown, _) => Err(DecodeError::UnknownVariant(unknown.to_owned())),
    }
}

pub(super) fn encode_cmp_op(op: CmpOp) -> Value {
    match op {
        CmpOp::Bool(bool::CmpOp::Eq) => json!(["EqOp"]),
        CmpOp::Bool(bool::CmpOp::Ne) => json!(["NeOp"]),
        CmpOp::Num(num::CmpOp::Lt) => json!(["LtOp"]),
        CmpOp::Num(num::CmpOp::Gt) => json!(["GtOp"]),
        CmpOp::Num(num::CmpOp::Le) => json!(["LeOp"]),
        CmpOp::Num(num::CmpOp::Ge) => json!(["GeOp"]),
    }
}

pub(super) fn decode_op_typ(value: &Value) -> Result<OpTyp, DecodeError> {
    let (tag, fields) = variant(value)?;
    match (tag, fields) {
        ("BoolT", []) => Ok(OpTyp::Bool),
        ("NatT", []) => Ok(OpTyp::Nat),
        ("IntT", []) => Ok(OpTyp::Int),
        ("BoolT" | "NatT" | "IntT", _) => {
            Err(DecodeError::Expected("valid IL operator type arity"))
        }
        (unknown, _) => Err(DecodeError::UnknownVariant(unknown.to_owned())),
    }
}

pub(super) fn encode_op_typ(typ: OpTyp) -> Value {
    match typ {
        OpTyp::Bool => json!(["BoolT"]),
        OpTyp::Nat => json!(["NatT"]),
        OpTyp::Int => json!(["IntT"]),
    }
}

pub(super) fn decode_subcheck(value: &Value) -> Result<ast::Subcheck, DecodeError> {
    let (tag, fields) = variant(value)?;
    match (tag, fields) {
        ("SkipSC", []) => Ok(ast::Subcheck::Skip),
        ("MixopSC", [mixops]) => Ok(ast::Subcheck::Mixop(decode_list(
            mixops,
            crate::wire::ocaml::mixfix::MixopCodec::decode,
        )?)),
        ("TupleSC", [subchecks]) => Ok(ast::Subcheck::Tuple(decode_list(
            subchecks,
            decode_subcheck,
        )?)),
        ("IterSC", [iter, subcheck]) => Ok(ast::Subcheck::Iter(
            decode_iter(iter)?,
            Box::new(decode_subcheck(subcheck)?),
        )),
        ("RecurseSC", [typ]) => Ok(ast::Subcheck::Recurse(decode_typ(typ)?)),
        ("SkipSC" | "MixopSC" | "TupleSC" | "IterSC" | "RecurseSC", _) => {
            Err(DecodeError::Expected("valid IL subtype check arity"))
        }
        (unknown, _) => Err(DecodeError::UnknownVariant(unknown.to_owned())),
    }
}

pub(super) fn encode_subcheck(subcheck: &ast::Subcheck) -> Value {
    match subcheck {
        ast::Subcheck::Skip => json!(["SkipSC"]),
        ast::Subcheck::Mixop(mixops) => json!([
            "MixopSC",
            encode_list(mixops, crate::wire::ocaml::mixfix::MixopCodec::encode)
        ]),
        ast::Subcheck::Tuple(subchecks) => {
            json!(["TupleSC", encode_list(subchecks, encode_subcheck)])
        }
        ast::Subcheck::Iter(iter, subcheck) => {
            json!(["IterSC", encode_iter(*iter), encode_subcheck(subcheck)])
        }
        ast::Subcheck::Recurse(typ) => json!(["RecurseSC", encode_typ(typ)]),
    }
}

pub(super) fn decode_exp(value: &Value) -> Result<ast::Exp, DecodeError> {
    let (kind, typ, span) = source::decode_annotated(value, decode_exp_kind, decode_typ_kind)?;
    Ok(Spanned::new(Noted::new(kind, typ), span))
}

pub(super) fn encode_exp(exp: &ast::Exp) -> Value {
    source::encode_annotated(
        &exp.node.kind,
        &exp.node.note,
        &exp.span,
        encode_exp_kind,
        encode_typ_kind,
    )
}

fn decode_exp_kind(value: &Value) -> Result<ExpKind, DecodeError> {
    let (tag, fields) = variant(value)?;
    match (tag, fields) {
        ("BoolE", [value]) => Ok(ExpKind::Bool(boolean(value)?)),
        ("NumE", [num]) => Ok(ExpKind::Num(xl::decode_num(num)?)),
        ("TextE", [text]) => Ok(ExpKind::Text(string(text)?.to_owned())),
        ("VarE", [id]) => Ok(ExpKind::Var(decode_id(id)?)),
        ("UnE", [op, typ, exp]) => Ok(ExpKind::Un(
            decode_un_op(op)?,
            decode_op_typ(typ)?,
            Box::new(decode_exp(exp)?),
        )),
        ("BinE", [op, typ, left, right]) => Ok(ExpKind::Bin(
            decode_bin_op(op)?,
            decode_op_typ(typ)?,
            Box::new(decode_exp(left)?),
            Box::new(decode_exp(right)?),
        )),
        ("CmpE", [op, typ, left, right]) => Ok(ExpKind::Cmp(
            decode_cmp_op(op)?,
            decode_op_typ(typ)?,
            Box::new(decode_exp(left)?),
            Box::new(decode_exp(right)?),
        )),
        ("UpCastE", [typ, exp]) => Ok(ExpKind::UpCast(
            decode_typ(typ)?,
            Box::new(decode_exp(exp)?),
        )),
        ("DownCastE", [typ, exp]) => Ok(ExpKind::DownCast(
            decode_typ(typ)?,
            Box::new(decode_exp(exp)?),
        )),
        ("SubE", [exp, typ, subcheck]) => Ok(ExpKind::Sub(
            Box::new(decode_exp(exp)?),
            decode_typ(typ)?,
            Box::new(decode_subcheck(subcheck)?),
        )),
        ("MatchE", [exp, pattern]) => Ok(ExpKind::Match(
            Box::new(decode_exp(exp)?),
            decode_pattern(pattern)?,
        )),
        ("TupleE", [exps]) => Ok(ExpKind::Tuple(decode_list(exps, decode_exp)?)),
        ("CaseE", [exp]) => Ok(ExpKind::Case(Box::new(decode_not_exp(exp)?))),
        ("StrE", [fields]) => Ok(ExpKind::Str(decode_list(fields, |field| {
            match array(field)? {
                [atom, exp] => Ok((AtomPhraseCodec::decode(atom)?, decode_exp(exp)?)),
                _ => Err(DecodeError::Expected("IL expression field pair")),
            }
        })?)),
        ("OptE", [exp]) => Ok(ExpKind::Opt(decode_option(exp, decode_exp)?.map(Box::new))),
        ("ListE", [exps]) => Ok(ExpKind::List(decode_list(exps, decode_exp)?)),
        ("ConsE", [head, tail]) => Ok(ExpKind::Cons(
            Box::new(decode_exp(head)?),
            Box::new(decode_exp(tail)?),
        )),
        ("CatE", [left, right]) => Ok(ExpKind::Cat(
            Box::new(decode_exp(left)?),
            Box::new(decode_exp(right)?),
        )),
        ("MemE", [left, right]) => Ok(ExpKind::Mem(
            Box::new(decode_exp(left)?),
            Box::new(decode_exp(right)?),
        )),
        ("LenE", [exp]) => Ok(ExpKind::Len(Box::new(decode_exp(exp)?))),
        ("DotE", [exp, atom]) => Ok(ExpKind::Dot(
            Box::new(decode_exp(exp)?),
            AtomPhraseCodec::decode(atom)?,
        )),
        ("IdxE", [base, index]) => Ok(ExpKind::Idx(
            Box::new(decode_exp(base)?),
            Box::new(decode_exp(index)?),
        )),
        ("SliceE", [base, left, right]) => Ok(ExpKind::Slice(
            Box::new(decode_exp(base)?),
            Box::new(decode_exp(left)?),
            Box::new(decode_exp(right)?),
        )),
        ("UpdE", [base, path, value]) => Ok(ExpKind::Upd(
            Box::new(decode_exp(base)?),
            decode_path(path)?,
            Box::new(decode_exp(value)?),
        )),
        ("CallE", [id, targs, args]) => Ok(ExpKind::Call(
            decode_id(id)?,
            decode_list(targs, decode_targ)?,
            decode_list(args, decode_arg)?,
        )),
        ("IterE", [exp, iter]) => Ok(ExpKind::Iter(
            Box::new(decode_exp(exp)?),
            decode_iter_exp(iter)?,
        )),
        (
            "BoolE" | "NumE" | "TextE" | "VarE" | "UnE" | "BinE" | "CmpE" | "UpCastE" | "DownCastE"
            | "SubE" | "MatchE" | "TupleE" | "CaseE" | "StrE" | "OptE" | "ListE" | "ConsE" | "CatE"
            | "MemE" | "LenE" | "DotE" | "IdxE" | "SliceE" | "UpdE" | "CallE" | "IterE",
            _,
        ) => Err(DecodeError::Expected("valid IL expression arity")),
        (unknown, _) => Err(DecodeError::UnknownVariant(unknown.to_owned())),
    }
}

fn encode_exp_kind(exp: &ExpKind) -> Value {
    match exp {
        ExpKind::Bool(value) => json!(["BoolE", value]),
        ExpKind::Num(num) => json!(["NumE", xl::encode_num(num)]),
        ExpKind::Text(text) => json!(["TextE", text]),
        ExpKind::Var(id) => json!(["VarE", encode_id(id)]),
        ExpKind::Un(op, typ, exp) => {
            json!([
                "UnE",
                encode_un_op(*op),
                encode_op_typ(*typ),
                encode_exp(exp)
            ])
        }
        ExpKind::Bin(op, typ, left, right) => json!([
            "BinE",
            encode_bin_op(*op),
            encode_op_typ(*typ),
            encode_exp(left),
            encode_exp(right)
        ]),
        ExpKind::Cmp(op, typ, left, right) => json!([
            "CmpE",
            encode_cmp_op(*op),
            encode_op_typ(*typ),
            encode_exp(left),
            encode_exp(right)
        ]),
        ExpKind::UpCast(typ, exp) => json!(["UpCastE", encode_typ(typ), encode_exp(exp)]),
        ExpKind::DownCast(typ, exp) => {
            json!(["DownCastE", encode_typ(typ), encode_exp(exp)])
        }
        ExpKind::Sub(exp, typ, subcheck) => json!([
            "SubE",
            encode_exp(exp),
            encode_typ(typ),
            encode_subcheck(subcheck)
        ]),
        ExpKind::Match(exp, pattern) => {
            json!(["MatchE", encode_exp(exp), encode_pattern(pattern)])
        }
        ExpKind::Tuple(exps) => json!(["TupleE", encode_list(exps, encode_exp)]),
        ExpKind::Case(exp) => json!(["CaseE", encode_not_exp(exp)]),
        ExpKind::Str(fields) => json!([
            "StrE",
            fields
                .iter()
                .map(|(atom, exp)| json!([AtomPhraseCodec::encode(atom), encode_exp(exp)]))
                .collect::<Vec<_>>()
        ]),
        ExpKind::Opt(exp) => json!(["OptE", encode_option(exp.as_deref(), encode_exp)]),
        ExpKind::List(exps) => json!(["ListE", encode_list(exps, encode_exp)]),
        ExpKind::Cons(head, tail) => json!(["ConsE", encode_exp(head), encode_exp(tail)]),
        ExpKind::Cat(left, right) => json!(["CatE", encode_exp(left), encode_exp(right)]),
        ExpKind::Mem(left, right) => json!(["MemE", encode_exp(left), encode_exp(right)]),
        ExpKind::Len(exp) => json!(["LenE", encode_exp(exp)]),
        ExpKind::Dot(exp, atom) => {
            json!(["DotE", encode_exp(exp), AtomPhraseCodec::encode(atom)])
        }
        ExpKind::Idx(base, index) => json!(["IdxE", encode_exp(base), encode_exp(index)]),
        ExpKind::Slice(base, left, right) => json!([
            "SliceE",
            encode_exp(base),
            encode_exp(left),
            encode_exp(right)
        ]),
        ExpKind::Upd(base, path, value) => {
            json!([
                "UpdE",
                encode_exp(base),
                encode_path(path),
                encode_exp(value)
            ])
        }
        ExpKind::Call(id, targs, args) => json!([
            "CallE",
            encode_id(id),
            encode_list(targs, encode_targ),
            encode_list(args, encode_arg)
        ]),
        ExpKind::Iter(exp, iter) => {
            json!(["IterE", encode_exp(exp), encode_iter_exp(iter)])
        }
    }
}

pub(super) fn decode_not_exp(value: &Value) -> Result<ast::NotExp, DecodeError> {
    mixfix::decode(value, decode_exp)
}

pub(super) fn encode_not_exp(exp: &ast::NotExp) -> Value {
    mixfix::encode(exp, encode_exp)
}

pub(super) fn decode_iter_exp(value: &Value) -> Result<ast::IterExp, DecodeError> {
    match array(value)? {
        [iter, vars] => Ok((decode_iter(iter)?, decode_list(vars, decode_var)?)),
        _ => Err(DecodeError::Expected("IL expression iterator pair")),
    }
}

pub(super) fn encode_iter_exp((iter, vars): &ast::IterExp) -> Value {
    json!([encode_iter(*iter), encode_list(vars, encode_var)])
}

pub(super) fn decode_pattern(value: &Value) -> Result<Pattern, DecodeError> {
    let (tag, fields) = variant(value)?;
    match (tag, fields) {
        ("CaseP", [mixop]) => Ok(Pattern::Case(Box::new(
            crate::wire::ocaml::mixfix::MixopCodec::decode(mixop)?,
        ))),
        ("ListP", [pattern]) => Ok(Pattern::List(decode_list_pattern(pattern)?)),
        ("OptP", [pattern]) => Ok(Pattern::Opt(decode_opt_pattern(pattern)?)),
        ("CaseP" | "ListP" | "OptP", _) => Err(DecodeError::Expected("valid IL pattern arity")),
        (unknown, _) => Err(DecodeError::UnknownVariant(unknown.to_owned())),
    }
}

pub(super) fn encode_pattern(pattern: &Pattern) -> Value {
    match pattern {
        Pattern::Case(mixop) => json!([
            "CaseP",
            crate::wire::ocaml::mixfix::MixopCodec::encode(mixop)
        ]),
        Pattern::List(pattern) => json!(["ListP", encode_list_pattern(pattern)]),
        Pattern::Opt(pattern) => json!(["OptP", encode_opt_pattern(*pattern)]),
    }
}

fn decode_list_pattern(value: &Value) -> Result<ListPattern, DecodeError> {
    let (tag, fields) = variant(value)?;
    match (tag, fields) {
        ("Cons", []) => Ok(ListPattern::Cons),
        ("Fixed", [length]) => Ok(ListPattern::Fixed(integer(length)?)),
        ("Nil", []) => Ok(ListPattern::Nil),
        ("Cons" | "Fixed" | "Nil", _) => Err(DecodeError::Expected("valid IL list pattern arity")),
        (unknown, _) => Err(DecodeError::UnknownVariant(unknown.to_owned())),
    }
}

fn encode_list_pattern(pattern: &ListPattern) -> Value {
    match pattern {
        ListPattern::Cons => json!(["Cons"]),
        ListPattern::Fixed(length) => json!(["Fixed", length]),
        ListPattern::Nil => json!(["Nil"]),
    }
}

fn decode_opt_pattern(value: &Value) -> Result<OptPattern, DecodeError> {
    let (tag, fields) = variant(value)?;
    match (tag, fields) {
        ("Some", []) => Ok(OptPattern::Some),
        ("None", []) => Ok(OptPattern::None),
        ("Some" | "None", _) => Err(DecodeError::Expected("valid IL option pattern arity")),
        (unknown, _) => Err(DecodeError::UnknownVariant(unknown.to_owned())),
    }
}

fn encode_opt_pattern(pattern: OptPattern) -> Value {
    match pattern {
        OptPattern::Some => json!(["Some"]),
        OptPattern::None => json!(["None"]),
    }
}

fn decode_path(value: &Value) -> Result<ast::Path, DecodeError> {
    let (kind, typ, span) = source::decode_annotated(value, decode_path_kind, decode_typ_kind)?;
    Ok(Spanned::new(Noted::new(kind, typ), span))
}

fn encode_path(path: &ast::Path) -> Value {
    source::encode_annotated(
        &path.node.kind,
        &path.node.note,
        &path.span,
        encode_path_kind,
        encode_typ_kind,
    )
}

fn decode_path_kind(value: &Value) -> Result<PathKind, DecodeError> {
    let (tag, fields) = variant(value)?;
    match (tag, fields) {
        ("RootP", []) => Ok(PathKind::Root),
        ("IdxP", [path, exp]) => Ok(PathKind::Idx(
            Box::new(decode_path(path)?),
            Box::new(decode_exp(exp)?),
        )),
        ("SliceP", [path, left, right]) => Ok(PathKind::Slice(
            Box::new(decode_path(path)?),
            Box::new(decode_exp(left)?),
            Box::new(decode_exp(right)?),
        )),
        ("DotP", [path, atom]) => Ok(PathKind::Dot(
            Box::new(decode_path(path)?),
            AtomPhraseCodec::decode(atom)?,
        )),
        ("RootP" | "IdxP" | "SliceP" | "DotP", _) => {
            Err(DecodeError::Expected("valid IL path arity"))
        }
        (unknown, _) => Err(DecodeError::UnknownVariant(unknown.to_owned())),
    }
}

fn encode_path_kind(path: &PathKind) -> Value {
    match path {
        PathKind::Root => json!(["RootP"]),
        PathKind::Idx(path, exp) => json!(["IdxP", encode_path(path), encode_exp(exp)]),
        PathKind::Slice(path, left, right) => json!([
            "SliceP",
            encode_path(path),
            encode_exp(left),
            encode_exp(right)
        ]),
        PathKind::Dot(path, atom) => {
            json!(["DotP", encode_path(path), AtomPhraseCodec::encode(atom)])
        }
    }
}

pub(super) fn decode_param(value: &Value) -> Result<ast::Param, DecodeError> {
    source::decode_phrase(value, |value| {
        let (tag, fields) = variant(value)?;
        match (tag, fields) {
            ("ExpP", [typ]) => Ok(ParamKind::Exp(decode_typ(typ)?)),
            ("DefP", [id, tparams, params, typ]) => Ok(ParamKind::Def(
                decode_id(id)?,
                decode_list(tparams, decode_tparam)?,
                decode_list(params, decode_param)?,
                decode_typ(typ)?,
            )),
            ("ExpP" | "DefP", _) => Err(DecodeError::Expected("valid IL parameter arity")),
            (unknown, _) => Err(DecodeError::UnknownVariant(unknown.to_owned())),
        }
    })
}

pub(super) fn encode_param(param: &ast::Param) -> Value {
    source::encode_phrase(param, |param| match param {
        ParamKind::Exp(typ) => json!(["ExpP", encode_typ(typ)]),
        ParamKind::Def(id, tparams, params, typ) => json!([
            "DefP",
            encode_id(id),
            encode_list(tparams, encode_tparam),
            encode_list(params, encode_param),
            encode_typ(typ)
        ]),
    })
}

pub(super) fn decode_arg(value: &Value) -> Result<ast::Arg, DecodeError> {
    source::decode_phrase(value, |value| {
        let (tag, fields) = variant(value)?;
        match (tag, fields) {
            ("ExpA", [exp]) => Ok(ArgKind::Exp(Box::new(decode_exp(exp)?))),
            ("DefA", [id]) => Ok(ArgKind::Def(decode_id(id)?)),
            ("ExpA" | "DefA", _) => Err(DecodeError::Expected("valid IL argument arity")),
            (unknown, _) => Err(DecodeError::UnknownVariant(unknown.to_owned())),
        }
    })
}

pub(super) fn encode_arg(arg: &ast::Arg) -> Value {
    source::encode_phrase(arg, |arg| match arg {
        ArgKind::Exp(exp) => json!(["ExpA", encode_exp(exp)]),
        ArgKind::Def(id) => json!(["DefA", encode_id(id)]),
    })
}

pub(super) fn decode_input_hint(
    value: &Value,
) -> Result<crate::lang::hints::input::InputHint, DecodeError> {
    Ok(crate::lang::hints::input::InputHint::new(decode_list(
        value, integer,
    )?))
}

pub(super) fn encode_input_hint(hint: &crate::lang::hints::input::InputHint) -> Value {
    encode_list(hint.indices(), |index| json!(index))
}

pub(super) fn decode_prem(value: &Value) -> Result<ast::Prem, DecodeError> {
    source::decode_phrase(value, |value| {
        let (tag, fields) = variant(value)?;
        match (tag, fields) {
            ("RulePr", [id, exp, hint]) => Ok(PremKind::Rule(RulePrem {
                id: decode_id(id)?,
                not_exp: decode_not_exp(exp)?,
                input_hint: decode_input_hint(hint)?,
            })),
            ("IfPr", [exp]) => Ok(PremKind::If(IfPrem {
                exp: decode_exp(exp)?,
            })),
            ("IfHoldPr", [id, exp]) => Ok(PremKind::IfHold(IfHoldPrem {
                id: decode_id(id)?,
                not_exp: decode_not_exp(exp)?,
            })),
            ("IfNotHoldPr", [id, exp]) => Ok(PremKind::IfNotHold(IfNotHoldPrem {
                id: decode_id(id)?,
                not_exp: decode_not_exp(exp)?,
            })),
            ("LetPr", [exp_l, exp_r]) => Ok(PremKind::Let(LetPrem {
                exp_l: decode_exp(exp_l)?,
                exp_r: decode_exp(exp_r)?,
            })),
            ("IterPr", [prem, iter]) => Ok(PremKind::Iter(IteratedPrem {
                prem: Box::new(decode_prem(prem)?),
                iter_prem: decode_iter_prem(iter)?,
            })),
            ("DebugPr", [exp]) => Ok(PremKind::Debug(DebugPrem {
                exp: decode_exp(exp)?,
            })),
            (
                "RulePr" | "IfPr" | "IfHoldPr" | "IfNotHoldPr" | "LetPr" | "IterPr" | "DebugPr",
                _,
            ) => Err(DecodeError::Expected("valid IL premise arity")),
            (unknown, _) => Err(DecodeError::UnknownVariant(unknown.to_owned())),
        }
    })
}

pub(super) fn encode_prem(prem: &ast::Prem) -> Value {
    source::encode_phrase(prem, |prem| match prem {
        PremKind::Rule(RulePrem {
            id,
            not_exp,
            input_hint: hint,
        }) => json!([
            "RulePr",
            encode_id(id),
            encode_not_exp(not_exp),
            encode_input_hint(hint)
        ]),
        PremKind::If(IfPrem { exp }) => json!(["IfPr", encode_exp(exp)]),
        PremKind::IfHold(IfHoldPrem { id, not_exp }) => {
            json!(["IfHoldPr", encode_id(id), encode_not_exp(not_exp)])
        }
        PremKind::IfNotHold(IfNotHoldPrem { id, not_exp }) => {
            json!(["IfNotHoldPr", encode_id(id), encode_not_exp(not_exp)])
        }
        PremKind::Let(LetPrem {
            exp_l: left,
            exp_r: right,
        }) => json!(["LetPr", encode_exp(left), encode_exp(right)]),
        PremKind::Iter(IteratedPrem {
            prem,
            iter_prem: iter,
        }) => {
            json!(["IterPr", encode_prem(prem), encode_iter_prem(iter)])
        }
        PremKind::Debug(DebugPrem { exp }) => json!(["DebugPr", encode_exp(exp)]),
    })
}

pub(super) fn decode_iter_prem(value: &Value) -> Result<ast::IterPrem, DecodeError> {
    match array(value)? {
        [iter, left, right] => Ok(ast::IterPrem {
            iter: decode_iter(iter)?,
            vars_bound: decode_list(left, decode_var)?,
            vars_bind: decode_list(right, decode_var)?,
        }),
        _ => Err(DecodeError::Expected("IL premise iterator triple")),
    }
}

pub(super) fn encode_iter_prem(iterprem: &ast::IterPrem) -> Value {
    json!([
        encode_iter(iterprem.iter),
        encode_list(&iterprem.vars_bound, encode_var),
        encode_list(&iterprem.vars_bind, encode_var)
    ])
}

fn decode_rule(value: &Value) -> Result<ast::Rule, DecodeError> {
    source::decode_phrase(value, |value| match array(value)? {
        [id, exp, prems] => Ok(ast::RuleKind {
            id: decode_id(id)?,
            not_exp: decode_not_exp(exp)?,
            prems: decode_list(prems, decode_prem)?,
        }),
        _ => Err(DecodeError::Expected("IL rule triple")),
    })
}

fn encode_rule(rule: &ast::Rule) -> Value {
    source::encode_phrase(rule, |rule| {
        json!([
            encode_id(&rule.id),
            encode_not_exp(&rule.not_exp),
            encode_list(&rule.prems, encode_prem)
        ])
    })
}

fn decode_rule_group(value: &Value) -> Result<ast::RuleGroup, DecodeError> {
    source::decode_phrase(value, |value| match array(value)? {
        [id, rules] => Ok((decode_id(id)?, decode_list(rules, decode_rule)?)),
        _ => Err(DecodeError::Expected("IL rule group pair")),
    })
}

fn encode_rule_group(group: &ast::RuleGroup) -> Value {
    source::encode_phrase(group, |(id, rules)| {
        json!([encode_id(id), encode_list(rules, encode_rule)])
    })
}

fn decode_else_group(value: &Value) -> Result<ast::ElseGroup, DecodeError> {
    source::decode_phrase(value, |value| match array(value)? {
        [id, rule] => Ok((decode_id(id)?, decode_rule(rule)?)),
        _ => Err(DecodeError::Expected("IL else group pair")),
    })
}

fn encode_else_group(group: &ast::ElseGroup) -> Value {
    source::encode_phrase(group, |(id, rule)| {
        json!([encode_id(id), encode_rule(rule)])
    })
}

pub(super) fn decode_clause(value: &Value) -> Result<ast::Clause, DecodeError> {
    source::decode_phrase(value, |value| match array(value)? {
        [args, exp, prems] => Ok(ast::ClauseKind {
            args: decode_list(args, decode_arg)?,
            expression: decode_exp(exp)?,
            premises: decode_list(prems, decode_prem)?,
        }),
        _ => Err(DecodeError::Expected("IL clause triple")),
    })
}

pub(super) fn encode_clause(clause: &ast::Clause) -> Value {
    source::encode_phrase(clause, |clause| {
        json!([
            encode_list(&clause.args, encode_arg),
            encode_exp(&clause.expression),
            encode_list(&clause.premises, encode_prem)
        ])
    })
}

fn decode_table_row(value: &Value) -> Result<ast::TableRow, DecodeError> {
    source::decode_phrase(value, |value| match array(value)? {
        [args, exp] => Ok((decode_list(args, decode_arg)?, decode_exp(exp)?)),
        _ => Err(DecodeError::Expected("IL table row pair")),
    })
}

fn encode_table_row(row: &ast::TableRow) -> Value {
    source::encode_phrase(row, |(args, exp)| {
        json!([encode_list(args, encode_arg), encode_exp(exp)])
    })
}

fn decode_def(value: &Value) -> Result<ast::Def, DecodeError> {
    source::decode_phrase(value, |value| {
        let (tag, fields) = variant(value)?;
        match (tag, fields) {
            ("ExternTypD", [id, hints]) => Ok(DefKind::ExternTyp(ExternTyp {
                id: decode_id(id)?,
                hints: decode_list(hints, el::decode_hint)?,
            })),
            ("TypD", [id, tparams, typ, hints]) => Ok(DefKind::Typ(TypDef {
                id: decode_id(id)?,
                tparams: decode_list(tparams, decode_tparam)?,
                def_typ: decode_def_typ(typ)?,
                hints: decode_list(hints, el::decode_hint)?,
            })),
            ("VarD", [id, typ, hints]) => Ok(DefKind::Var(VarDef {
                id: decode_id(id)?,
                typ: decode_typ(typ)?,
                hints: decode_list(hints, el::decode_hint)?,
            })),
            ("ExternRelD", [id, typ, input, hints]) => Ok(DefKind::ExternRel(ExternRel {
                id: decode_id(id)?,
                not_typ: decode_not_typ(typ)?,
                input_hint: decode_input_hint(input)?,
                hints: decode_list(hints, el::decode_hint)?,
            })),
            ("RelD", [id, typ, input, groups, else_group, hints]) => Ok(DefKind::Rel(Rel {
                id: decode_id(id)?,
                not_typ: decode_not_typ(typ)?,
                input_hint: decode_input_hint(input)?,
                rule_groups: decode_list(groups, decode_rule_group)?,
                else_group: decode_option(else_group, decode_else_group)?,
                hints: decode_list(hints, el::decode_hint)?,
            })),
            ("ExternDecD", [id, tparams, params, typ, hints]) => {
                Ok(DefKind::ExternDec(ExternDec {
                    id: decode_id(id)?,
                    tparams: decode_list(tparams, decode_tparam)?,
                    params: decode_list(params, decode_param)?,
                    typ: decode_typ(typ)?,
                    hints: decode_list(hints, el::decode_hint)?,
                }))
            }
            ("BuiltinDecD", [id, tparams, params, typ, hints]) => {
                Ok(DefKind::BuiltinDec(BuiltinDec {
                    id: decode_id(id)?,
                    tparams: decode_list(tparams, decode_tparam)?,
                    params: decode_list(params, decode_param)?,
                    typ: decode_typ(typ)?,
                    hints: decode_list(hints, el::decode_hint)?,
                }))
            }
            ("TableDecD", [id, params, typ, rows, hints]) => Ok(DefKind::TableDec(TableDec {
                id: decode_id(id)?,
                params: decode_list(params, decode_param)?,
                typ: decode_typ(typ)?,
                rows: decode_list(rows, decode_table_row)?,
                hints: decode_list(hints, el::decode_hint)?,
            })),
            ("FuncDecD", [id, tparams, params, typ, clauses, else_clause, hints]) => {
                Ok(DefKind::FuncDec(FuncDec {
                    id: decode_id(id)?,
                    tparams: decode_list(tparams, decode_tparam)?,
                    params: decode_list(params, decode_param)?,
                    typ: decode_typ(typ)?,
                    clauses: decode_list(clauses, decode_clause)?,
                    else_clause: decode_option(else_clause, decode_clause)?,
                    hints: decode_list(hints, el::decode_hint)?,
                }))
            }
            (
                "ExternTypD" | "TypD" | "VarD" | "ExternRelD" | "RelD" | "ExternDecD"
                | "BuiltinDecD" | "TableDecD" | "FuncDecD",
                _,
            ) => Err(DecodeError::Expected("valid IL definition arity")),
            (unknown, _) => Err(DecodeError::UnknownVariant(unknown.to_owned())),
        }
    })
}

fn encode_def(def: &ast::Def) -> Value {
    source::encode_phrase(def, |def| match def {
        DefKind::ExternTyp(ExternTyp { id, hints }) => json!([
            "ExternTypD",
            encode_id(id),
            encode_list(hints, el::encode_hint)
        ]),
        DefKind::Typ(TypDef {
            id,
            tparams,
            def_typ: typ,
            hints,
        }) => json!([
            "TypD",
            encode_id(id),
            encode_list(tparams, encode_tparam),
            encode_def_typ(typ),
            encode_list(hints, el::encode_hint)
        ]),
        DefKind::Var(VarDef { id, typ, hints }) => json!([
            "VarD",
            encode_id(id),
            encode_typ(typ),
            encode_list(hints, el::encode_hint)
        ]),
        DefKind::ExternRel(ExternRel {
            id,
            not_typ: typ,
            input_hint: input,
            hints,
        }) => json!([
            "ExternRelD",
            encode_id(id),
            encode_not_typ(typ),
            encode_input_hint(input),
            encode_list(hints, el::encode_hint)
        ]),
        DefKind::Rel(Rel {
            id,
            not_typ: typ,
            input_hint: input,
            rule_groups: groups,
            else_group,
            hints,
        }) => json!([
            "RelD",
            encode_id(id),
            encode_not_typ(typ),
            encode_input_hint(input),
            encode_list(groups, encode_rule_group),
            encode_option(else_group.as_ref(), encode_else_group),
            encode_list(hints, el::encode_hint)
        ]),
        DefKind::ExternDec(ExternDec {
            id,
            tparams,
            params,
            typ,
            hints,
        }) => json!([
            "ExternDecD",
            encode_id(id),
            encode_list(tparams, encode_tparam),
            encode_list(params, encode_param),
            encode_typ(typ),
            encode_list(hints, el::encode_hint)
        ]),
        DefKind::BuiltinDec(BuiltinDec {
            id,
            tparams,
            params,
            typ,
            hints,
        }) => json!([
            "BuiltinDecD",
            encode_id(id),
            encode_list(tparams, encode_tparam),
            encode_list(params, encode_param),
            encode_typ(typ),
            encode_list(hints, el::encode_hint)
        ]),
        DefKind::TableDec(TableDec {
            id,
            params,
            typ,
            rows,
            hints,
        }) => json!([
            "TableDecD",
            encode_id(id),
            encode_list(params, encode_param),
            encode_typ(typ),
            encode_list(rows, encode_table_row),
            encode_list(hints, el::encode_hint)
        ]),
        DefKind::FuncDec(FuncDec {
            id,
            tparams,
            params,
            typ,
            clauses,
            else_clause,
            hints,
        }) => json!([
            "FuncDecD",
            encode_id(id),
            encode_list(tparams, encode_tparam),
            encode_list(params, encode_param),
            encode_typ(typ),
            encode_list(clauses, encode_clause),
            encode_option(else_clause.as_ref(), encode_clause),
            encode_list(hints, el::encode_hint)
        ]),
    })
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::{decode_subcheck, encode_subcheck};

    #[test]
    fn subtype_check_operations_round_trip_ocaml_yojson_shapes() {
        let typ = json!({
            "it": ["BoolT"],
            "note": null,
            "at": {
                "left": {"file": "", "line": 0, "column": 0},
                "right": {"file": "", "line": 0, "column": 0}
            }
        });
        let operations = [
            json!(["SkipSC"]),
            json!(["MixopSC", [["Atom", {
                "it": ["Keyword", "NUM"],
                "note": null,
                "at": {
                    "left": {"file": "", "line": 0, "column": 0},
                    "right": {"file": "", "line": 0, "column": 0}
                }
            }]]]),
            json!(["TupleSC", [["SkipSC"], ["RecurseSC", typ]]]),
            json!(["IterSC", ["List"], ["SkipSC"]]),
        ];

        for operation in operations {
            let decoded = decode_subcheck(&operation).expect("decode subtype check");
            assert_eq!(encode_subcheck(&decoded), operation);
        }
    }
}
