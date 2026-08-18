use serde_json::{Value, json};

use crate::lang::sl::ast::{self, DefKind, Guard, HoldCase, InstrKind, ParamKind};

use super::{
    super::{
        DecodeError, EncodeError, array, boolean, field, integer, object, on_codec_stack, variant,
    },
    el, il,
};
use crate::wire::ocaml::source;

pub struct SpecCodec;

impl SpecCodec {
    pub fn decode(value: &Value) -> Result<ast::Spec, DecodeError> {
        on_codec_stack(|| il::decode_list(value, decode_def))
    }

    pub fn encode(spec: &ast::Spec) -> Result<Value, EncodeError> {
        on_codec_stack(|| Ok(il::encode_list(spec, encode_def)))
    }
}

fn decode_option<T>(
    value: &Value,
    decode: impl FnOnce(&Value) -> Result<T, DecodeError>,
) -> Result<Option<T>, DecodeError> {
    if value.is_null() {
        Ok(None)
    } else {
        Ok(Some(decode(value)?))
    }
}

fn encode_option<T>(value: Option<&T>, encode: impl FnOnce(&T) -> Value) -> Value {
    value.map_or(Value::Null, encode)
}

fn decode_param(value: &Value) -> Result<ast::Param, DecodeError> {
    source::decode_phrase(value, |value| {
        let (tag, fields) = variant(value)?;
        match (tag, fields) {
            ("ExpP", [typ, exp]) => Ok(ParamKind::ExpP(il::decode_typ(typ)?, il::decode_exp(exp)?)),
            ("DefP", [id, tparams, params, typ]) => Ok(ParamKind::DefP(
                il::decode_id(id)?,
                il::decode_list(tparams, il::decode_tparam)?,
                il::decode_list(params, decode_param)?,
                il::decode_typ(typ)?,
            )),
            ("ExpP" | "DefP", _) => Err(DecodeError::Expected("valid SL parameter arity")),
            (unknown, _) => Err(DecodeError::UnknownVariant(unknown.to_owned())),
        }
    })
}

fn encode_param(param: &ast::Param) -> Value {
    source::encode_phrase(param, |param| match param {
        ParamKind::ExpP(typ, exp) => {
            json!(["ExpP", il::encode_typ(typ), il::encode_exp(exp)])
        }
        ParamKind::DefP(id, tparams, params, typ) => json!([
            "DefP",
            il::encode_id(id),
            il::encode_list(tparams, il::encode_tparam),
            il::encode_list(params, encode_param),
            il::encode_typ(typ)
        ]),
    })
}

fn decode_hold_case(value: &Value) -> Result<HoldCase, DecodeError> {
    let (tag, fields) = variant(value)?;
    match (tag, fields) {
        ("BothH", [left, right]) => Ok(HoldCase::BothH(decode_block(left)?, decode_block(right)?)),
        ("HoldH", [block, dangle]) => Ok(HoldCase::HoldH(decode_block(block)?, boolean(dangle)?)),
        ("NotHoldH", [block, dangle]) => {
            Ok(HoldCase::NotHoldH(decode_block(block)?, boolean(dangle)?))
        }
        ("BothH" | "HoldH" | "NotHoldH", _) => {
            Err(DecodeError::Expected("valid SL hold case arity"))
        }
        (unknown, _) => Err(DecodeError::UnknownVariant(unknown.to_owned())),
    }
}

fn encode_hold_case(case: &HoldCase) -> Value {
    match case {
        HoldCase::BothH(left, right) => {
            json!(["BothH", encode_block(left), encode_block(right)])
        }
        HoldCase::HoldH(block, dangle) => {
            json!(["HoldH", encode_block(block), dangle])
        }
        HoldCase::NotHoldH(block, dangle) => {
            json!(["NotHoldH", encode_block(block), dangle])
        }
    }
}

fn decode_case(value: &Value) -> Result<ast::Case, DecodeError> {
    match array(value)? {
        [guard, block] => Ok((decode_guard(guard)?, decode_block(block)?)),
        _ => Err(DecodeError::Expected("SL case pair")),
    }
}

fn encode_case((guard, block): &ast::Case) -> Value {
    json!([encode_guard(guard), encode_block(block)])
}

fn decode_guard(value: &Value) -> Result<Guard, DecodeError> {
    let (tag, fields) = variant(value)?;
    match (tag, fields) {
        ("BoolG", [value]) => Ok(Guard::BoolG(boolean(value)?)),
        ("CmpG", [op, typ, exp]) => Ok(Guard::CmpG(
            il::decode_cmp_op(op)?,
            il::decode_op_typ(typ)?,
            il::decode_exp(exp)?,
        )),
        ("SubG", [typ]) => Ok(Guard::SubG(il::decode_typ(typ)?)),
        ("MatchG", [pattern]) => Ok(Guard::MatchG(il::decode_pattern(pattern)?)),
        ("MemG", [exp]) => Ok(Guard::MemG(il::decode_exp(exp)?)),
        ("BoolG" | "CmpG" | "SubG" | "MatchG" | "MemG", _) => {
            Err(DecodeError::Expected("valid SL guard arity"))
        }
        (unknown, _) => Err(DecodeError::UnknownVariant(unknown.to_owned())),
    }
}

fn encode_guard(guard: &Guard) -> Value {
    match guard {
        Guard::BoolG(value) => json!(["BoolG", value]),
        Guard::CmpG(op, typ, exp) => json!([
            "CmpG",
            il::encode_cmp_op(*op),
            il::encode_op_typ(*typ),
            il::encode_exp(exp)
        ]),
        Guard::SubG(typ) => json!(["SubG", il::encode_typ(typ)]),
        Guard::MatchG(pattern) => json!(["MatchG", il::encode_pattern(pattern)]),
        Guard::MemG(exp) => json!(["MemG", il::encode_exp(exp)]),
    }
}

fn decode_iid(value: &Value) -> Result<ast::Iid, DecodeError> {
    let object = object(value)?;
    integer(field(object, "iid")?)
}

fn encode_iid(iid: &ast::Iid) -> Value {
    json!({"iid": iid})
}

fn decode_instr(value: &Value) -> Result<ast::Instr, DecodeError> {
    let (kind, iid, span) = source::decode_annotated(value, decode_instr_kind, decode_iid)?;
    Ok(ast::Instr::new(kind, iid, span))
}

fn encode_instr(instr: &ast::Instr) -> Value {
    source::encode_annotated(
        &instr.kind,
        &instr.iid,
        &instr.span,
        encode_instr_kind,
        encode_iid,
    )
}

fn decode_instr_kind(value: &Value) -> Result<InstrKind, DecodeError> {
    let (tag, fields) = variant(value)?;
    match (tag, fields) {
        ("IfI", [exp, iters, block, dangle]) => Ok(InstrKind::IfI(
            il::decode_exp(exp)?,
            il::decode_list(iters, il::decode_iter_exp)?,
            decode_block(block)?,
            boolean(dangle)?,
        )),
        ("HoldI", [id, exp, iters, case]) => Ok(InstrKind::HoldI(
            il::decode_id(id)?,
            il::decode_not_exp(exp)?,
            il::decode_list(iters, il::decode_iter_exp)?,
            decode_hold_case(case)?,
        )),
        ("CaseI", [exp, cases, dangle]) => Ok(InstrKind::CaseI(
            il::decode_exp(exp)?,
            il::decode_list(cases, decode_case)?,
            boolean(dangle)?,
        )),
        ("GroupI", [id, signature, exps, block]) => Ok(InstrKind::GroupI(
            il::decode_id(id)?,
            decode_rel_signature(signature)?,
            il::decode_list(exps, il::decode_exp)?,
            decode_block(block)?,
        )),
        ("LetI", [left, right, iters, block]) => Ok(InstrKind::LetI(
            il::decode_exp(left)?,
            il::decode_exp(right)?,
            il::decode_list(iters, il::decode_iter_prem)?,
            decode_block(block)?,
        )),
        ("RuleI", [id, exp, input, iters, block]) => Ok(InstrKind::RuleI(
            il::decode_id(id)?,
            il::decode_not_exp(exp)?,
            il::decode_input_hint(input)?,
            il::decode_list(iters, il::decode_iter_prem)?,
            decode_block(block)?,
        )),
        ("ResultI", [signature, exps]) => Ok(InstrKind::ResultI(
            decode_rel_signature(signature)?,
            il::decode_list(exps, il::decode_exp)?,
        )),
        ("ReturnI", [exp]) => Ok(InstrKind::ReturnI(il::decode_exp(exp)?)),
        ("DebugI", [exp, instr]) => Ok(InstrKind::DebugI(
            il::decode_exp(exp)?,
            Box::new(decode_instr(instr)?),
        )),
        (
            "IfI" | "HoldI" | "CaseI" | "GroupI" | "LetI" | "RuleI" | "ResultI" | "ReturnI"
            | "DebugI",
            _,
        ) => Err(DecodeError::Expected("valid SL instruction arity")),
        (unknown, _) => Err(DecodeError::UnknownVariant(unknown.to_owned())),
    }
}

fn encode_instr_kind(instr: &InstrKind) -> Value {
    match instr {
        InstrKind::IfI(exp, iters, block, dangle) => json!([
            "IfI",
            il::encode_exp(exp),
            il::encode_list(iters, il::encode_iter_exp),
            encode_block(block),
            dangle
        ]),
        InstrKind::HoldI(id, exp, iters, case) => json!([
            "HoldI",
            il::encode_id(id),
            il::encode_not_exp(exp),
            il::encode_list(iters, il::encode_iter_exp),
            encode_hold_case(case)
        ]),
        InstrKind::CaseI(exp, cases, dangle) => json!([
            "CaseI",
            il::encode_exp(exp),
            il::encode_list(cases, encode_case),
            dangle
        ]),
        InstrKind::GroupI(id, signature, exps, block) => json!([
            "GroupI",
            il::encode_id(id),
            encode_rel_signature(signature),
            il::encode_list(exps, il::encode_exp),
            encode_block(block)
        ]),
        InstrKind::LetI(left, right, iters, block) => json!([
            "LetI",
            il::encode_exp(left),
            il::encode_exp(right),
            il::encode_list(iters, il::encode_iter_prem),
            encode_block(block)
        ]),
        InstrKind::RuleI(id, exp, input, iters, block) => json!([
            "RuleI",
            il::encode_id(id),
            il::encode_not_exp(exp),
            il::encode_input_hint(input),
            il::encode_list(iters, il::encode_iter_prem),
            encode_block(block)
        ]),
        InstrKind::ResultI(signature, exps) => json!([
            "ResultI",
            encode_rel_signature(signature),
            il::encode_list(exps, il::encode_exp)
        ]),
        InstrKind::ReturnI(exp) => json!(["ReturnI", il::encode_exp(exp)]),
        InstrKind::DebugI(exp, instr) => {
            json!(["DebugI", il::encode_exp(exp), encode_instr(instr)])
        }
    }
}

fn decode_block(value: &Value) -> Result<ast::Block, DecodeError> {
    il::decode_list(value, decode_instr)
}

fn encode_block(block: &ast::Block) -> Value {
    il::encode_list(block, encode_instr)
}

fn decode_rel_signature(value: &Value) -> Result<ast::RelSignature, DecodeError> {
    match array(value)? {
        [typ, input] => Ok((il::decode_not_typ(typ)?, il::decode_input_hint(input)?)),
        _ => Err(DecodeError::Expected("SL relation signature pair")),
    }
}

fn encode_rel_signature((typ, input): &ast::RelSignature) -> Value {
    json!([il::encode_not_typ(typ), il::encode_input_hint(input)])
}

fn decode_extern_rel(value: &Value) -> Result<ast::ExternRel, DecodeError> {
    match array(value)? {
        [id, signature, exps, hints] => Ok((
            il::decode_id(id)?,
            decode_rel_signature(signature)?,
            il::decode_list(exps, il::decode_exp)?,
            il::decode_list(hints, el::decode_hint)?,
        )),
        _ => Err(DecodeError::Expected("SL external relation tuple")),
    }
}

fn encode_extern_rel((id, signature, exps, hints): &ast::ExternRel) -> Value {
    json!([
        il::encode_id(id),
        encode_rel_signature(signature),
        il::encode_list(exps, il::encode_exp),
        il::encode_list(hints, el::encode_hint)
    ])
}

fn decode_rel(value: &Value) -> Result<ast::Rel, DecodeError> {
    match array(value)? {
        [id, signature, exps, block, else_block, hints] => Ok((
            il::decode_id(id)?,
            decode_rel_signature(signature)?,
            il::decode_list(exps, il::decode_exp)?,
            decode_block(block)?,
            decode_option(else_block, decode_block)?,
            il::decode_list(hints, el::decode_hint)?,
        )),
        _ => Err(DecodeError::Expected("SL relation tuple")),
    }
}

fn encode_rel((id, signature, exps, block, else_block, hints): &ast::Rel) -> Value {
    json!([
        il::encode_id(id),
        encode_rel_signature(signature),
        il::encode_list(exps, il::encode_exp),
        encode_block(block),
        encode_option(else_block.as_ref(), encode_block),
        il::encode_list(hints, el::encode_hint)
    ])
}

fn decode_extern_func(value: &Value) -> Result<ast::ExternFunc, DecodeError> {
    match array(value)? {
        [id, tparams, params, typ, hints] => Ok((
            il::decode_id(id)?,
            il::decode_list(tparams, il::decode_tparam)?,
            il::decode_list(params, decode_param)?,
            il::decode_typ(typ)?,
            il::decode_list(hints, el::decode_hint)?,
        )),
        _ => Err(DecodeError::Expected("SL external function tuple")),
    }
}

fn encode_extern_func((id, tparams, params, typ, hints): &ast::ExternFunc) -> Value {
    json!([
        il::encode_id(id),
        il::encode_list(tparams, il::encode_tparam),
        il::encode_list(params, encode_param),
        il::encode_typ(typ),
        il::encode_list(hints, el::encode_hint)
    ])
}

fn decode_table_row(value: &Value) -> Result<ast::TableRow, DecodeError> {
    match array(value)? {
        [args, exp, block] => Ok((
            il::decode_list(args, il::decode_exp)?,
            il::decode_exp(exp)?,
            decode_block(block)?,
        )),
        _ => Err(DecodeError::Expected("SL table row triple")),
    }
}

fn encode_table_row((args, exp, block): &ast::TableRow) -> Value {
    json!([
        il::encode_list(args, il::encode_exp),
        il::encode_exp(exp),
        encode_block(block)
    ])
}

fn decode_table_func(value: &Value) -> Result<ast::TableFunc, DecodeError> {
    match array(value)? {
        [id, params, typ, rows, hints] => Ok((
            il::decode_id(id)?,
            il::decode_list(params, decode_param)?,
            il::decode_typ(typ)?,
            il::decode_list(rows, decode_table_row)?,
            il::decode_list(hints, el::decode_hint)?,
        )),
        _ => Err(DecodeError::Expected("SL table function tuple")),
    }
}

fn encode_table_func((id, params, typ, rows, hints): &ast::TableFunc) -> Value {
    json!([
        il::encode_id(id),
        il::encode_list(params, encode_param),
        il::encode_typ(typ),
        il::encode_list(rows, encode_table_row),
        il::encode_list(hints, el::encode_hint)
    ])
}

fn decode_defined_func(value: &Value) -> Result<ast::DefinedFunc, DecodeError> {
    match array(value)? {
        [id, tparams, params, typ, block, else_block, hints] => Ok((
            il::decode_id(id)?,
            il::decode_list(tparams, il::decode_tparam)?,
            il::decode_list(params, decode_param)?,
            il::decode_typ(typ)?,
            decode_block(block)?,
            decode_option(else_block, decode_block)?,
            il::decode_list(hints, el::decode_hint)?,
        )),
        _ => Err(DecodeError::Expected("SL defined function tuple")),
    }
}

fn encode_defined_func(
    (id, tparams, params, typ, block, else_block, hints): &ast::DefinedFunc,
) -> Value {
    json!([
        il::encode_id(id),
        il::encode_list(tparams, il::encode_tparam),
        il::encode_list(params, encode_param),
        il::encode_typ(typ),
        encode_block(block),
        encode_option(else_block.as_ref(), encode_block),
        il::encode_list(hints, el::encode_hint)
    ])
}

fn decode_def(value: &Value) -> Result<ast::Def, DecodeError> {
    source::decode_phrase(value, |value| {
        let (tag, fields) = variant(value)?;
        match (tag, fields) {
            ("ExternTypD", [id, hints]) => Ok(DefKind::ExternTypD(
                il::decode_id(id)?,
                il::decode_list(hints, el::decode_hint)?,
            )),
            ("TypD", [id, tparams, typ, hints]) => Ok(DefKind::TypD(
                il::decode_id(id)?,
                il::decode_list(tparams, il::decode_tparam)?,
                super::il::decode_def_typ(typ)?,
                il::decode_list(hints, el::decode_hint)?,
            )),
            ("VarD", [id, typ, hints]) => Ok(DefKind::VarD(
                il::decode_id(id)?,
                il::decode_typ(typ)?,
                il::decode_list(hints, el::decode_hint)?,
            )),
            ("ExternRelD", [rel]) => Ok(DefKind::ExternRelD(decode_extern_rel(rel)?)),
            ("RelD", [rel]) => Ok(DefKind::RelD(decode_rel(rel)?)),
            ("ExternDecD", [func]) => Ok(DefKind::ExternDecD(decode_extern_func(func)?)),
            ("BuiltinDecD", [func]) => Ok(DefKind::BuiltinDecD(decode_extern_func(func)?)),
            ("TableDecD", [func]) => Ok(DefKind::TableDecD(decode_table_func(func)?)),
            ("FuncDecD", [func]) => Ok(DefKind::FuncDecD(decode_defined_func(func)?)),
            (
                "ExternTypD" | "TypD" | "VarD" | "ExternRelD" | "RelD" | "ExternDecD"
                | "BuiltinDecD" | "TableDecD" | "FuncDecD",
                _,
            ) => Err(DecodeError::Expected("valid SL definition arity")),
            (unknown, _) => Err(DecodeError::UnknownVariant(unknown.to_owned())),
        }
    })
}

fn encode_def(def: &ast::Def) -> Value {
    source::encode_phrase(def, |def| match def {
        DefKind::ExternTypD(id, hints) => json!([
            "ExternTypD",
            il::encode_id(id),
            il::encode_list(hints, el::encode_hint)
        ]),
        DefKind::TypD(id, tparams, typ, hints) => json!([
            "TypD",
            il::encode_id(id),
            il::encode_list(tparams, il::encode_tparam),
            super::il::encode_def_typ(typ),
            il::encode_list(hints, el::encode_hint)
        ]),
        DefKind::VarD(id, typ, hints) => json!([
            "VarD",
            il::encode_id(id),
            il::encode_typ(typ),
            il::encode_list(hints, el::encode_hint)
        ]),
        DefKind::ExternRelD(rel) => json!(["ExternRelD", encode_extern_rel(rel)]),
        DefKind::RelD(rel) => json!(["RelD", encode_rel(rel)]),
        DefKind::ExternDecD(func) => json!(["ExternDecD", encode_extern_func(func)]),
        DefKind::BuiltinDecD(func) => json!(["BuiltinDecD", encode_extern_func(func)]),
        DefKind::TableDecD(func) => json!(["TableDecD", encode_table_func(func)]),
        DefKind::FuncDecD(func) => json!(["FuncDecD", encode_defined_func(func)]),
    })
}
