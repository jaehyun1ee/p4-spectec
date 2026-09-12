pub(super) use crate::wire::ocaml::typ::*;

/// OCaml-compatible JSON codecs for IL data
use std::cell::Cell;

use serde_json::json;

use crate::util::json::json;
use thiserror::Error;

use crate::lang::data::value::{ValueArena, make};
use crate::lang::{
    il::ast::{self, *},
    xl::{bool, num},
};

use super::{
    super::{
        DecodeError, EncodeError, array, boolean, field, integer, object, on_codec_stack, string,
        variant,
    },
    el, xl,
};
use crate::wire::VALUE_SCHEMA;
use crate::wire::ocaml::{atom::AtomPhraseCodec, mixfix, reader, source};

/// Codec for complete IL specifications
pub struct SpecCodec;

impl SpecCodec {
    pub fn decode(json: &json) -> Result<ast::Spec, DecodeError> {
        on_codec_stack(|| decode_list(json, decode_def))
    }

    pub fn encode(spec: &ast::Spec) -> Result<json, EncodeError> {
        on_codec_stack(|| Ok(encode_list(spec, encode_def)))
    }
}

pub struct ValueCodec;

/// Standard JSON codec for IL values
///
/// # Panics
///
/// Encoding or decoding an extern payload panics, including nested payloads
/// Native extern state has no OCaml wire representation
impl ValueCodec {
    pub fn decode(arena: &mut ValueArena, json: &json) -> Result<ast::Value, DecodeError> {
        on_codec_stack(|| decode_value(arena, json))
    }

    pub fn encode(arena: &ValueArena, value: &ast::Value) -> Result<json, EncodeError> {
        on_codec_stack(|| {
            ValueEncoder {
                arena,
                next_vid: Cell::new(0),
            }
            .encode_value(value)
        })
    }
}

/// Standard JSON codec for the versioned OCaml IL value envelope
///
/// Rejects duplicate keys, non-standard JSON and floating numeric tokens
/// Language integers use decimal strings; annotation integers must fit i64
///
/// # Panics
///
/// Encoding or decoding an extern payload panics, including nested payloads
pub struct ValueEnvelopeCodec;

impl ValueEnvelopeCodec {
    pub fn decode(
        arena: &mut ValueArena,
        input: &[u8],
    ) -> Result<ast::Value, ValueEnvelopeDecodeError> {
        on_codec_stack(|| {
            let json_envelope = reader::from_slice(input)?;
            let fields = object(&json_envelope)?;
            let schema = string(field(fields, "schema")?)?;
            let kind = string(field(fields, "kind")?)?;

            if schema != VALUE_SCHEMA {
                return Err(ValueEnvelopeDecodeError::UnknownSchema(schema.to_owned()));
            }
            if kind != "value" {
                return Err(ValueEnvelopeDecodeError::SchemaKindMismatch(
                    kind.to_owned(),
                ));
            }

            decode_value(arena, field(fields, "payload")?).map_err(Into::into)
        })
    }

    pub fn encode(
        arena: &ValueArena,
        value: &ast::Value,
    ) -> Result<Vec<u8>, ValueEnvelopeEncodeError> {
        on_codec_stack(|| {
            let json_payload = ValueCodec::encode(arena, value)?;
            let json_envelope = json!({
                "schema": VALUE_SCHEMA,
                "kind": "value",
                "payload": json_payload,
            });
            serde_json::to_vec(&json_envelope).map_err(Into::into)
        })
    }
}

#[derive(Debug, Error)]
pub enum ValueEnvelopeDecodeError {
    #[error("invalid JSON value envelope: {0}")]
    Parse(#[from] serde_json::Error),

    #[error("invalid OCaml IL value: {0}")]
    Decode(#[from] DecodeError),

    #[error("unknown wire schema `{0}`")]
    UnknownSchema(String),

    #[error("schema `p4spectec.value.v1` requires kind `value`, but found `{0}`")]
    SchemaKindMismatch(String),
}

#[derive(Debug, Error)]
pub enum ValueEnvelopeEncodeError {
    #[error(transparent)]
    Encode(#[from] EncodeError),

    #[error("cannot write JSON value envelope: {0}")]
    Write(#[from] serde_json::Error),
}

pub(super) fn decode_list<T>(
    json: &json,
    decode: impl FnMut(&json) -> Result<T, DecodeError>,
) -> Result<Vec<T>, DecodeError> {
    array(json)?.iter().map(decode).collect()
}

pub(super) fn encode_list<T>(values: &[T], encode: impl Fn(&T) -> json) -> json {
    json::Array(values.iter().map(encode).collect())
}

pub(super) fn decode_option<T>(
    json: &json,
    decode: impl FnOnce(&json) -> Result<T, DecodeError>,
) -> Result<Option<T>, DecodeError> {
    if json.is_null() {
        Ok(None)
    } else {
        Ok(Some(decode(json)?))
    }
}

pub(super) fn encode_option<T>(value: Option<&T>, encode: impl FnOnce(&T) -> json) -> json {
    value.map_or(json::Null, encode)
}

pub(super) fn decode_var(json: &json) -> Result<ast::Var, DecodeError> {
    match array(json)? {
        [id, typ, iters] => Ok(ast::Var {
            id: decode_id(id)?,
            typ: decode_typ(typ)?,
            iters: decode_list(iters, decode_iter)?,
        }),
        _ => Err(DecodeError::Expected("IL variable triple")),
    }
}

pub(super) fn encode_var(variable: &ast::Var) -> json {
    json!([
        encode_id(&variable.id),
        encode_typ(&variable.typ),
        encode_list(&variable.iters, |iter| encode_iter(*iter))
    ])
}

pub(super) fn decode_not_typ(json: &json) -> Result<ast::NotTyp, DecodeError> {
    source::decode_phrase(json, |json| mixfix::decode(json, decode_typ))
}

pub(super) fn encode_not_typ(typ: &ast::NotTyp) -> json {
    source::encode_phrase(typ, |typ| mixfix::encode(typ, encode_typ))
}

fn decode_typ_origin(json: &json) -> Result<ast::TypOrigin, DecodeError> {
    source::decode_phrase(json, |json| match array(json)? {
        [id, targs] => Ok((decode_id(id)?, decode_list(targs, decode_targ)?)),
        _ => Err(DecodeError::Expected("IL type origin pair")),
    })
}

fn encode_typ_origin(origin: &ast::TypOrigin) -> json {
    source::encode_phrase(origin, |(id, targs)| {
        json!([encode_id(id), encode_list(targs, encode_targ)])
    })
}

pub(super) fn decode_def_typ(json: &json) -> Result<ast::DefTyp, DecodeError> {
    source::decode_phrase(json, |json| {
        let (tag, fields) = variant(json)?;
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

pub(super) fn encode_def_typ(typ: &ast::DefTyp) -> json {
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

fn decode_vnote(json: &json) -> Result<TypKind, DecodeError> {
    let object = object(json)?;
    integer(field(object, "vid")?)?;
    let typ = decode_typ_kind(field(object, "typ")?)?;
    integer(field(object, "vhash")?)?;
    Ok(typ)
}

struct ValueEncoder<'a> {
    arena: &'a ValueArena,
    next_vid: Cell<i64>,
}

impl ValueEncoder<'_> {
    fn encode_vnote(&self, typ: &TypKind) -> json {
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

fn decode_value(arena: &mut ValueArena, json: &json) -> Result<ast::Value, DecodeError> {
    let object = object(json)?;
    let node = decode_value_kind(arena, field(object, "it")?)?;
    let typ = decode_vnote(field(object, "note")?)?;
    let span = source::decode_region(field(object, "at")?)?;
    Ok(make::new(arena, node, typ.into(), span)?)
}

fn decode_value_kind(arena: &mut ValueArena, json: &json) -> Result<ValueKind, DecodeError> {
    let (tag, fields) = variant(json)?;
    match (tag, fields) {
        ("BoolV", [json]) => Ok(ValueKind::Bool(boolean(json)?)),
        ("NumV", [num]) => Ok(ValueKind::Num(xl::decode_num(num)?)),
        ("TextV", [text]) => Ok(ValueKind::Text(string(text)?.to_owned())),
        ("StructV", [fields]) => Ok(ValueKind::Struct(decode_list(
            fields,
            |field| match array(field)? {
                [atom, json] => Ok((AtomPhraseCodec::decode(atom)?, decode_value(arena, json)?)),
                _ => Err(DecodeError::Expected("IL value field pair")),
            },
        )?)),
        ("CaseV", [case]) => Ok(ValueKind::Case(mixfix::decode(case, |json| {
            decode_value(arena, json)
        })?)),
        ("TupleV", [json]) => Ok(ValueKind::Tuple(decode_list(json, |json| {
            decode_value(arena, json)
        })?)),
        ("OptV", [json]) => Ok(ValueKind::Opt(decode_option(json, |json| {
            decode_value(arena, json)
        })?)),
        ("ListV", [json]) => Ok(ValueKind::List(decode_list(json, |json| {
            decode_value(arena, json)
        })?)),
        ("FuncV", [id]) => Ok(ValueKind::Func(decode_id(id)?)),
        ("ExternV", [_]) => panic!("extern payloads are not supported by OCaml wire"),
        (
            "BoolV" | "NumV" | "TextV" | "StructV" | "CaseV" | "TupleV" | "OptV" | "ListV"
            | "FuncV" | "ExternV",
            _,
        ) => Err(DecodeError::Expected("valid IL value arity")),
        (unknown, _) => Err(DecodeError::UnknownVariant(unknown.to_owned())),
    }
}

impl ValueEncoder<'_> {
    fn encode_value(&self, value: &ast::Value) -> Result<json, EncodeError> {
        let kind = self.encode_value_kind(self.arena.kind(value))?;
        Ok(json!({
            "it": kind,
            "note": self.encode_vnote(self.arena.typ(value)),
            "at": source::encode_region(self.arena.span(value)),
        }))
    }

    fn encode_value_kind(&self, value_kind: &ValueKind) -> Result<json, EncodeError> {
        Ok(match value_kind {
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
                    None => json::Null,
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
            ValueKind::Extern(_) => panic!("extern payloads are not supported by OCaml wire"),
        })
    }
}

pub(super) fn decode_un_op(json: &json) -> Result<UnOp, DecodeError> {
    let (tag, fields) = variant(json)?;
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

pub(super) fn encode_un_op(op: UnOp) -> json {
    match op {
        UnOp::Bool(bool::UnOp::Not) => json!(["NotOp"]),
        UnOp::Num(num::UnOp::Plus) => json!(["PlusOp"]),
        UnOp::Num(num::UnOp::Minus) => json!(["MinusOp"]),
    }
}

pub(super) fn decode_bin_op(json: &json) -> Result<BinOp, DecodeError> {
    let (tag, fields) = variant(json)?;
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

pub(super) fn encode_bin_op(op: BinOp) -> json {
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

pub(super) fn decode_cmp_op(json: &json) -> Result<CmpOp, DecodeError> {
    let (tag, fields) = variant(json)?;
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

pub(super) fn encode_cmp_op(op: CmpOp) -> json {
    match op {
        CmpOp::Bool(bool::CmpOp::Eq) => json!(["EqOp"]),
        CmpOp::Bool(bool::CmpOp::Ne) => json!(["NeOp"]),
        CmpOp::Num(num::CmpOp::Lt) => json!(["LtOp"]),
        CmpOp::Num(num::CmpOp::Gt) => json!(["GtOp"]),
        CmpOp::Num(num::CmpOp::Le) => json!(["LeOp"]),
        CmpOp::Num(num::CmpOp::Ge) => json!(["GeOp"]),
    }
}

pub(super) fn decode_op_typ(json: &json) -> Result<OpTyp, DecodeError> {
    let (tag, fields) = variant(json)?;
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

pub(super) fn encode_op_typ(typ: OpTyp) -> json {
    match typ {
        OpTyp::Bool => json!(["BoolT"]),
        OpTyp::Nat => json!(["NatT"]),
        OpTyp::Int => json!(["IntT"]),
    }
}

pub(super) fn decode_subcheck(json: &json) -> Result<ast::Subcheck, DecodeError> {
    let (tag, fields) = variant(json)?;
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

pub(super) fn encode_subcheck(subcheck: &ast::Subcheck) -> json {
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

pub(super) fn decode_exp(json: &json) -> Result<ast::Exp, DecodeError> {
    source::decode_note_phrase(json, decode_exp_kind, |json| {
        decode_typ_kind(json).map(Into::into)
    })
}

pub(super) fn encode_exp(exp: &ast::Exp) -> json {
    source::encode_note_phrase(exp, encode_exp_kind, |note| encode_typ_kind(note))
}

fn decode_exp_kind(json: &json) -> Result<ExpKind, DecodeError> {
    let (tag, fields) = variant(json)?;
    match (tag, fields) {
        ("BoolE", [json]) => Ok(ExpKind::Bool(boolean(json)?)),
        ("NumE", [num]) => Ok(ExpKind::Num(xl::decode_num(num)?)),
        ("TextE", [text]) => Ok(ExpKind::Text(string(text)?.to_owned())),
        ("VarE", [id]) => Ok(ExpKind::Var(decode_id(id)?)),
        ("UnE", [op, typ, exp]) => Ok(ExpKind::Un(
            decode_un_op(op)?,
            decode_op_typ(typ)?,
            Box::new(decode_exp(exp)?),
        )),
        ("BinE", [op, typ, exp_l, exp_r]) => Ok(ExpKind::Bin(
            decode_bin_op(op)?,
            decode_op_typ(typ)?,
            Box::new(decode_exp(exp_l)?),
            Box::new(decode_exp(exp_r)?),
        )),
        ("CmpE", [op, typ, exp_l, exp_r]) => Ok(ExpKind::Cmp(
            decode_cmp_op(op)?,
            decode_op_typ(typ)?,
            Box::new(decode_exp(exp_l)?),
            Box::new(decode_exp(exp_r)?),
        )),
        ("UpCastE", [typ, exp]) => Ok(ExpKind::UpCast(
            Box::new(decode_typ(typ)?),
            Box::new(decode_exp(exp)?),
        )),
        ("DownCastE", [typ, exp]) => Ok(ExpKind::DownCast(
            Box::new(decode_typ(typ)?),
            Box::new(decode_exp(exp)?),
        )),
        ("SubE", [exp, typ, subcheck]) => Ok(ExpKind::Sub(
            Box::new(decode_exp(exp)?),
            Box::new(decode_typ(typ)?),
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
        ("ConsE", [exp_head, exp_tail]) => Ok(ExpKind::Cons(
            Box::new(decode_exp(exp_head)?),
            Box::new(decode_exp(exp_tail)?),
        )),
        ("CatE", [exp_l, exp_r]) => Ok(ExpKind::Cat(
            Box::new(decode_exp(exp_l)?),
            Box::new(decode_exp(exp_r)?),
        )),
        ("MemE", [exp_l, exp_r]) => Ok(ExpKind::Mem(
            Box::new(decode_exp(exp_l)?),
            Box::new(decode_exp(exp_r)?),
        )),
        ("LenE", [exp]) => Ok(ExpKind::Len(Box::new(decode_exp(exp)?))),
        ("DotE", [exp, atom]) => Ok(ExpKind::Dot(
            Box::new(decode_exp(exp)?),
            AtomPhraseCodec::decode(atom)?,
        )),
        ("IdxE", [exp_base, exp_idx]) => Ok(ExpKind::Idx(
            Box::new(decode_exp(exp_base)?),
            Box::new(decode_exp(exp_idx)?),
        )),
        ("SliceE", [exp_base, exp_idx, exp_len]) => Ok(ExpKind::Slice(
            Box::new(decode_exp(exp_base)?),
            Box::new(decode_exp(exp_idx)?),
            Box::new(decode_exp(exp_len)?),
        )),
        ("UpdE", [exp_base, path, exp_field]) => Ok(ExpKind::Upd(
            Box::new(decode_exp(exp_base)?),
            Box::new(decode_path(path)?),
            Box::new(decode_exp(exp_field)?),
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

fn encode_exp_kind(exp: &ExpKind) -> json {
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
        ExpKind::Bin(op, typ, exp_l, exp_r) => json!([
            "BinE",
            encode_bin_op(*op),
            encode_op_typ(*typ),
            encode_exp(exp_l),
            encode_exp(exp_r)
        ]),
        ExpKind::Cmp(op, typ, exp_l, exp_r) => json!([
            "CmpE",
            encode_cmp_op(*op),
            encode_op_typ(*typ),
            encode_exp(exp_l),
            encode_exp(exp_r)
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
        ExpKind::Cons(exp_head, exp_tail) => {
            json!(["ConsE", encode_exp(exp_head), encode_exp(exp_tail)])
        }
        ExpKind::Cat(exp_l, exp_r) => json!(["CatE", encode_exp(exp_l), encode_exp(exp_r)]),
        ExpKind::Mem(exp_l, exp_r) => json!(["MemE", encode_exp(exp_l), encode_exp(exp_r)]),
        ExpKind::Len(exp) => json!(["LenE", encode_exp(exp)]),
        ExpKind::Dot(exp, atom) => {
            json!(["DotE", encode_exp(exp), AtomPhraseCodec::encode(atom)])
        }
        ExpKind::Idx(exp_base, exp_idx) => {
            json!(["IdxE", encode_exp(exp_base), encode_exp(exp_idx)])
        }
        ExpKind::Slice(exp_base, exp_idx, exp_len) => json!([
            "SliceE",
            encode_exp(exp_base),
            encode_exp(exp_idx),
            encode_exp(exp_len)
        ]),
        ExpKind::Upd(exp_base, path, exp_field) => {
            json!([
                "UpdE",
                encode_exp(exp_base),
                encode_path(path),
                encode_exp(exp_field)
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

pub(super) fn decode_not_exp(json: &json) -> Result<ast::NotExp, DecodeError> {
    mixfix::decode(json, decode_exp)
}

pub(super) fn encode_not_exp(exp: &ast::NotExp) -> json {
    mixfix::encode(exp, encode_exp)
}

pub(super) fn decode_iter_exp(json: &json) -> Result<ast::ExpIter, DecodeError> {
    match array(json)? {
        [iter, vars] => Ok((decode_iter(iter)?, decode_list(vars, decode_var)?)),
        _ => Err(DecodeError::Expected("IL expression iterator pair")),
    }
}

pub(super) fn encode_iter_exp((iter, vars): &ast::ExpIter) -> json {
    json!([encode_iter(*iter), encode_list(vars, encode_var)])
}

pub(super) fn decode_pattern(json: &json) -> Result<Pattern, DecodeError> {
    let (tag, fields) = variant(json)?;
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

pub(super) fn encode_pattern(pattern: &Pattern) -> json {
    match pattern {
        Pattern::Case(mixop) => json!([
            "CaseP",
            crate::wire::ocaml::mixfix::MixopCodec::encode(mixop)
        ]),
        Pattern::List(pattern) => json!(["ListP", encode_list_pattern(pattern)]),
        Pattern::Opt(pattern) => json!(["OptP", encode_opt_pattern(*pattern)]),
    }
}

fn decode_list_pattern(json: &json) -> Result<ListPattern, DecodeError> {
    let (tag, fields) = variant(json)?;
    match (tag, fields) {
        ("Cons", []) => Ok(ListPattern::Cons),
        ("Fixed", [length]) => Ok(ListPattern::Fixed(integer(length)?)),
        ("Nil", []) => Ok(ListPattern::Nil),
        ("Cons" | "Fixed" | "Nil", _) => Err(DecodeError::Expected("valid IL list pattern arity")),
        (unknown, _) => Err(DecodeError::UnknownVariant(unknown.to_owned())),
    }
}

fn encode_list_pattern(pattern: &ListPattern) -> json {
    match pattern {
        ListPattern::Cons => json!(["Cons"]),
        ListPattern::Fixed(length) => json!(["Fixed", length]),
        ListPattern::Nil => json!(["Nil"]),
    }
}

fn decode_opt_pattern(json: &json) -> Result<OptPattern, DecodeError> {
    let (tag, fields) = variant(json)?;
    match (tag, fields) {
        ("Some", []) => Ok(OptPattern::Some),
        ("None", []) => Ok(OptPattern::None),
        ("Some" | "None", _) => Err(DecodeError::Expected("valid IL option pattern arity")),
        (unknown, _) => Err(DecodeError::UnknownVariant(unknown.to_owned())),
    }
}

fn encode_opt_pattern(pattern: OptPattern) -> json {
    match pattern {
        OptPattern::Some => json!(["Some"]),
        OptPattern::None => json!(["None"]),
    }
}

fn decode_path(json: &json) -> Result<ast::Path, DecodeError> {
    source::decode_note_phrase(json, decode_path_kind, |json| {
        decode_typ_kind(json).map(Into::into)
    })
}

fn encode_path(path: &ast::Path) -> json {
    source::encode_note_phrase(path, encode_path_kind, |note| encode_typ_kind(note))
}

fn decode_path_kind(json: &json) -> Result<PathKind, DecodeError> {
    let (tag, fields) = variant(json)?;
    match (tag, fields) {
        ("RootP", []) => Ok(PathKind::Root),
        ("IdxP", [path, exp]) => Ok(PathKind::Idx(
            Box::new(decode_path(path)?),
            Box::new(decode_exp(exp)?),
        )),
        ("SliceP", [path, exp_idx, exp_len]) => Ok(PathKind::Slice(
            Box::new(decode_path(path)?),
            Box::new(decode_exp(exp_idx)?),
            Box::new(decode_exp(exp_len)?),
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

fn encode_path_kind(path: &PathKind) -> json {
    match path {
        PathKind::Root => json!(["RootP"]),
        PathKind::Idx(path, exp) => json!(["IdxP", encode_path(path), encode_exp(exp)]),
        PathKind::Slice(path, exp_idx, exp_len) => json!([
            "SliceP",
            encode_path(path),
            encode_exp(exp_idx),
            encode_exp(exp_len)
        ]),
        PathKind::Dot(path, atom) => {
            json!(["DotP", encode_path(path), AtomPhraseCodec::encode(atom)])
        }
    }
}

pub(super) fn decode_param(json: &json) -> Result<ast::Param, DecodeError> {
    source::decode_phrase(json, |json| {
        let (tag, fields) = variant(json)?;
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

pub(super) fn encode_param(param: &ast::Param) -> json {
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

pub(super) fn decode_arg(json: &json) -> Result<ast::Arg, DecodeError> {
    source::decode_phrase(json, |json| {
        let (tag, fields) = variant(json)?;
        match (tag, fields) {
            ("ExpA", [exp]) => Ok(ArgKind::Exp(Box::new(decode_exp(exp)?))),
            ("DefA", [id]) => Ok(ArgKind::Def(decode_id(id)?)),
            ("ExpA" | "DefA", _) => Err(DecodeError::Expected("valid IL argument arity")),
            (unknown, _) => Err(DecodeError::UnknownVariant(unknown.to_owned())),
        }
    })
}

pub(super) fn encode_arg(arg: &ast::Arg) -> json {
    source::encode_phrase(arg, |arg| match arg {
        ArgKind::Exp(exp) => json!(["ExpA", encode_exp(exp)]),
        ArgKind::Def(id) => json!(["DefA", encode_id(id)]),
    })
}

pub(super) fn decode_input_hint(
    json: &json,
) -> Result<crate::lang::hints::input::InputHint, DecodeError> {
    Ok(crate::lang::hints::input::InputHint::new(decode_list(
        json, integer,
    )?))
}

pub(super) fn encode_input_hint(hint: &crate::lang::hints::input::InputHint) -> json {
    encode_list(hint.indices(), |idx| json!(idx))
}

pub(super) fn decode_prem(json: &json) -> Result<ast::Prem, DecodeError> {
    source::decode_phrase(json, |json| {
        let (tag, fields) = variant(json)?;
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
            ("IterPr", [prem, prem_iter]) => Ok(PremKind::Iter(IterPrem {
                prem: Box::new(decode_prem(prem)?),
                prem_iter: decode_prem_iter(prem_iter)?,
            })),
            ("DebugPr", [exp]) => Ok(PremKind::Debug(DebugPrem {
                exp: decode_exp(exp)?,
            })),
            ("RulePr" | "IfPr" | "IfHoldPr" | "IfNotHoldPr" | "IterPr" | "DebugPr", _) => {
                Err(DecodeError::Expected("valid IL premise arity"))
            }
            (unknown, _) => Err(DecodeError::UnknownVariant(unknown.to_owned())),
        }
    })
}

pub(super) fn encode_prem(prem: &ast::Prem) -> json {
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
        PremKind::Iter(IterPrem { prem, prem_iter }) => {
            json!(["IterPr", encode_prem(prem), encode_prem_iter(prem_iter)])
        }
        PremKind::Debug(DebugPrem { exp }) => json!(["DebugPr", encode_exp(exp)]),
    })
}

pub(super) fn decode_prem_iter(json: &json) -> Result<ast::PremIter, DecodeError> {
    match array(json)? {
        [iter, vars_bound, vars_bind] => Ok(ast::PremIter {
            iter: decode_iter(iter)?,
            vars_bound: decode_list(vars_bound, decode_var)?,
            vars_bind: decode_list(vars_bind, decode_var)?,
        }),
        _ => Err(DecodeError::Expected("IL premise iterator triple")),
    }
}

pub(super) fn encode_prem_iter(prem_iter: &ast::PremIter) -> json {
    json!([
        encode_iter(prem_iter.iter),
        encode_list(&prem_iter.vars_bound, encode_var),
        encode_list(&prem_iter.vars_bind, encode_var)
    ])
}

fn decode_rule(json: &json) -> Result<ast::Rule, DecodeError> {
    source::decode_phrase(json, |json| match array(json)? {
        [id, exp, prems] => Ok(ast::RuleKind {
            id: decode_id(id)?,
            not_exp: decode_not_exp(exp)?,
            prems: decode_list(prems, decode_prem)?,
        }),
        _ => Err(DecodeError::Expected("IL rule triple")),
    })
}

fn encode_rule(rule: &ast::Rule) -> json {
    source::encode_phrase(rule, |rule| {
        json!([
            encode_id(&rule.id),
            encode_not_exp(&rule.not_exp),
            encode_list(&rule.prems, encode_prem)
        ])
    })
}

fn decode_rule_group(json: &json) -> Result<ast::RuleGroup, DecodeError> {
    source::decode_phrase(json, |json| match array(json)? {
        [id, rules] => Ok((decode_id(id)?, decode_list(rules, decode_rule)?)),
        _ => Err(DecodeError::Expected("IL rule group pair")),
    })
}

fn encode_rule_group(group: &ast::RuleGroup) -> json {
    source::encode_phrase(group, |(id, rules)| {
        json!([encode_id(id), encode_list(rules, encode_rule)])
    })
}

fn decode_else_group(json: &json) -> Result<ast::ElseGroup, DecodeError> {
    source::decode_phrase(json, |json| match array(json)? {
        [id, rule] => Ok((decode_id(id)?, decode_rule(rule)?)),
        _ => Err(DecodeError::Expected("IL else group pair")),
    })
}

fn encode_else_group(group: &ast::ElseGroup) -> json {
    source::encode_phrase(group, |(id, rule)| {
        json!([encode_id(id), encode_rule(rule)])
    })
}

pub(super) fn decode_clause(json: &json) -> Result<ast::Clause, DecodeError> {
    source::decode_phrase(json, |json| match array(json)? {
        [args, exp, prems] => Ok(ast::ClauseKind {
            args: decode_list(args, decode_arg)?,
            exp: decode_exp(exp)?,
            prems: decode_list(prems, decode_prem)?,
        }),
        _ => Err(DecodeError::Expected("IL clause triple")),
    })
}

pub(super) fn encode_clause(clause: &ast::Clause) -> json {
    source::encode_phrase(clause, |clause| {
        json!([
            encode_list(&clause.args, encode_arg),
            encode_exp(&clause.exp),
            encode_list(&clause.prems, encode_prem)
        ])
    })
}

fn decode_table_row(json: &json) -> Result<ast::TableRow, DecodeError> {
    source::decode_phrase(json, |json| match array(json)? {
        [args, exp] => Ok((decode_list(args, decode_arg)?, decode_exp(exp)?)),
        _ => Err(DecodeError::Expected("IL table row pair")),
    })
}

fn encode_table_row(row: &ast::TableRow) -> json {
    source::encode_phrase(row, |(args, exp)| {
        json!([encode_list(args, encode_arg), encode_exp(exp)])
    })
}

fn decode_def(json: &json) -> Result<ast::Def, DecodeError> {
    source::decode_phrase(json, |json| {
        let (tag, fields) = variant(json)?;
        match (tag, fields) {
            ("ExternTypD", [id, hints]) => Ok(DefKind::Typ(TypDef::Extern(ExternTyp {
                id: decode_id(id)?,
                hints: decode_list(hints, el::decode_hint)?,
            }))),
            ("TypD", [id, tparams, typ, hints]) => {
                Ok(DefKind::Typ(TypDef::Defined(Box::new(DefinedTyp {
                    id: decode_id(id)?,
                    tparams: decode_list(tparams, decode_tparam)?,
                    def_typ: decode_def_typ(typ)?,
                    hints: decode_list(hints, el::decode_hint)?,
                }))))
            }
            ("VarD", [id, typ, hints]) => Ok(DefKind::Var(VarDef {
                id: decode_id(id)?,
                typ: decode_typ(typ)?,
                hints: decode_list(hints, el::decode_hint)?,
            })),
            ("ExternRelD", [id, typ, input, hints]) => {
                Ok(DefKind::Rel(RelDef::Extern(Box::new(ExternRel {
                    id: decode_id(id)?,
                    not_typ: decode_not_typ(typ)?,
                    input_hint: decode_input_hint(input)?,
                    hints: decode_list(hints, el::decode_hint)?,
                }))))
            }
            ("RelD", [id, typ, input, groups, else_group, hints]) => {
                Ok(DefKind::Rel(RelDef::Defined(Box::new(DefinedRel {
                    id: decode_id(id)?,
                    not_typ: decode_not_typ(typ)?,
                    input_hint: decode_input_hint(input)?,
                    rule_groups: decode_list(groups, decode_rule_group)?,
                    else_group: decode_option(else_group, decode_else_group)?,
                    hints: decode_list(hints, el::decode_hint)?,
                }))))
            }
            ("ExternDecD", [id, tparams, params, typ, hints]) => {
                Ok(DefKind::MetaFunc(MetaFuncDef::Extern(ExternFunc {
                    id: decode_id(id)?,
                    tparams: decode_list(tparams, decode_tparam)?,
                    params: decode_list(params, decode_param)?,
                    typ: decode_typ(typ)?,
                    hints: decode_list(hints, el::decode_hint)?,
                })))
            }
            ("BuiltinDecD", [id, tparams, params, typ, hints]) => {
                Ok(DefKind::MetaFunc(MetaFuncDef::Builtin(BuiltinFunc {
                    id: decode_id(id)?,
                    tparams: decode_list(tparams, decode_tparam)?,
                    params: decode_list(params, decode_param)?,
                    typ: decode_typ(typ)?,
                    hints: decode_list(hints, el::decode_hint)?,
                })))
            }
            ("TableDecD", [id, params, typ, rows, hints]) => {
                Ok(DefKind::MetaFunc(MetaFuncDef::Table(TableFunc {
                    id: decode_id(id)?,
                    params: decode_list(params, decode_param)?,
                    typ: decode_typ(typ)?,
                    rows: decode_list(rows, decode_table_row)?,
                    hints: decode_list(hints, el::decode_hint)?,
                })))
            }
            ("FuncDecD", [id, tparams, params, typ, clauses, else_clause, hints]) => Ok(
                DefKind::MetaFunc(MetaFuncDef::Defined(Box::new(DefinedFunc {
                    id: decode_id(id)?,
                    tparams: decode_list(tparams, decode_tparam)?,
                    params: decode_list(params, decode_param)?,
                    typ: decode_typ(typ)?,
                    clauses: decode_list(clauses, decode_clause)?,
                    else_clause: decode_option(else_clause, decode_clause)?,
                    hints: decode_list(hints, el::decode_hint)?,
                }))),
            ),
            (
                "ExternTypD" | "TypD" | "VarD" | "ExternRelD" | "RelD" | "ExternDecD"
                | "BuiltinDecD" | "TableDecD" | "FuncDecD",
                _,
            ) => Err(DecodeError::Expected("valid IL definition arity")),
            (unknown, _) => Err(DecodeError::UnknownVariant(unknown.to_owned())),
        }
    })
}

fn encode_typ_def(typ_def_il: &TypDef) -> json {
    match typ_def_il {
        TypDef::Extern(extern_typ_il) => json!([
            "ExternTypD",
            encode_id(&extern_typ_il.id),
            encode_list(&extern_typ_il.hints, el::encode_hint)
        ]),
        TypDef::Defined(defined_typ_il) => json!([
            "TypD",
            encode_id(&defined_typ_il.id),
            encode_list(&defined_typ_il.tparams, encode_tparam),
            encode_def_typ(&defined_typ_il.def_typ),
            encode_list(&defined_typ_il.hints, el::encode_hint)
        ]),
    }
}

fn encode_var_def(var_def_il: &VarDef) -> json {
    json!([
        "VarD",
        encode_id(&var_def_il.id),
        encode_typ(&var_def_il.typ),
        encode_list(&var_def_il.hints, el::encode_hint)
    ])
}

fn encode_rel_def(rel_def_il: &RelDef) -> json {
    match rel_def_il {
        RelDef::Extern(extern_rel_il) => json!([
            "ExternRelD",
            encode_id(&extern_rel_il.id),
            encode_not_typ(&extern_rel_il.not_typ),
            encode_input_hint(&extern_rel_il.input_hint),
            encode_list(&extern_rel_il.hints, el::encode_hint)
        ]),
        RelDef::Defined(defined_rel_il) => json!([
            "RelD",
            encode_id(&defined_rel_il.id),
            encode_not_typ(&defined_rel_il.not_typ),
            encode_input_hint(&defined_rel_il.input_hint),
            encode_list(&defined_rel_il.rule_groups, encode_rule_group),
            encode_option(defined_rel_il.else_group.as_ref(), encode_else_group),
            encode_list(&defined_rel_il.hints, el::encode_hint)
        ]),
    }
}

fn encode_meta_func_def(meta_func_def_il: &MetaFuncDef) -> json {
    match meta_func_def_il {
        MetaFuncDef::Extern(extern_func_il) => json!([
            "ExternDecD",
            encode_id(&extern_func_il.id),
            encode_list(&extern_func_il.tparams, encode_tparam),
            encode_list(&extern_func_il.params, encode_param),
            encode_typ(&extern_func_il.typ),
            encode_list(&extern_func_il.hints, el::encode_hint)
        ]),
        MetaFuncDef::Builtin(builtin_func_il) => json!([
            "BuiltinDecD",
            encode_id(&builtin_func_il.id),
            encode_list(&builtin_func_il.tparams, encode_tparam),
            encode_list(&builtin_func_il.params, encode_param),
            encode_typ(&builtin_func_il.typ),
            encode_list(&builtin_func_il.hints, el::encode_hint)
        ]),
        MetaFuncDef::Table(table_func_il) => json!([
            "TableDecD",
            encode_id(&table_func_il.id),
            encode_list(&table_func_il.params, encode_param),
            encode_typ(&table_func_il.typ),
            encode_list(&table_func_il.rows, encode_table_row),
            encode_list(&table_func_il.hints, el::encode_hint)
        ]),
        MetaFuncDef::Defined(defined_func_il) => json!([
            "FuncDecD",
            encode_id(&defined_func_il.id),
            encode_list(&defined_func_il.tparams, encode_tparam),
            encode_list(&defined_func_il.params, encode_param),
            encode_typ(&defined_func_il.typ),
            encode_list(&defined_func_il.clauses, encode_clause),
            encode_option(defined_func_il.else_clause.as_ref(), encode_clause),
            encode_list(&defined_func_il.hints, el::encode_hint)
        ]),
    }
}

fn encode_def(def_il: &ast::Def) -> json {
    source::encode_phrase(def_il, |def_kind_il| match def_kind_il {
        DefKind::Typ(typ_def_il) => encode_typ_def(typ_def_il),
        DefKind::Var(var_def_il) => encode_var_def(var_def_il),
        DefKind::Rel(rel_def_il) => encode_rel_def(rel_def_il),
        DefKind::MetaFunc(meta_func_def_il) => encode_meta_func_def(meta_func_def_il),
    })
}
