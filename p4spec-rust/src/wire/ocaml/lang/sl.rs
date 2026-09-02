use serde_json::{Value, json};

use crate::lang::sl::ast::{self, *};

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
            ("ExpP", [typ, exp]) => Ok(ParamKind::Exp(
                il::decode_typ(typ)?,
                Box::new(il::decode_exp(exp)?),
            )),
            ("DefP", [id, tparams, params, typ]) => Ok(ParamKind::Def(
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
        ParamKind::Exp(typ, exp) => {
            json!(["ExpP", il::encode_typ(typ), il::encode_exp(exp)])
        }
        ParamKind::Def(id, tparams, params, typ) => json!([
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
        ("BothH", [left, right]) => Ok(HoldCase::Both(decode_block(left)?, decode_block(right)?)),
        ("HoldH", [block, dangle]) => Ok(HoldCase::Hold(decode_block(block)?, boolean(dangle)?)),
        ("NotHoldH", [block, dangle]) => {
            Ok(HoldCase::NotHold(decode_block(block)?, boolean(dangle)?))
        }
        ("BothH" | "HoldH" | "NotHoldH", _) => {
            Err(DecodeError::Expected("valid SL hold case arity"))
        }
        (unknown, _) => Err(DecodeError::UnknownVariant(unknown.to_owned())),
    }
}

fn encode_hold_case(case: &HoldCase) -> Value {
    match case {
        HoldCase::Both(left, right) => {
            json!(["BothH", encode_block(left), encode_block(right)])
        }
        HoldCase::Hold(block, dangle) => {
            json!(["HoldH", encode_block(block), dangle])
        }
        HoldCase::NotHold(block, dangle) => {
            json!(["NotHoldH", encode_block(block), dangle])
        }
    }
}

fn decode_case(value: &Value) -> Result<ast::Case, DecodeError> {
    match array(value)? {
        [guard, block] => Ok(ast::Case {
            guard: decode_guard(guard)?,
            block: decode_block(block)?,
        }),
        _ => Err(DecodeError::Expected("SL case pair")),
    }
}

fn encode_case(case: &ast::Case) -> Value {
    json!([encode_guard(&case.guard), encode_block(&case.block)])
}

fn decode_guard(value: &Value) -> Result<Guard, DecodeError> {
    let (tag, fields) = variant(value)?;
    match (tag, fields) {
        ("BoolG", [value]) => Ok(Guard::Bool(boolean(value)?)),
        ("CmpG", [op, typ, exp]) => Ok(Guard::Cmp(
            il::decode_cmp_op(op)?,
            il::decode_op_typ(typ)?,
            il::decode_exp(exp)?,
        )),
        ("SubG", [typ, subcheck]) => Ok(Guard::Sub(
            il::decode_typ(typ)?,
            Box::new(il::decode_subcheck(subcheck)?),
        )),
        ("MatchG", [pattern]) => Ok(Guard::Match(il::decode_pattern(pattern)?)),
        ("MemG", [exp]) => Ok(Guard::Mem(il::decode_exp(exp)?)),
        ("BoolG" | "CmpG" | "SubG" | "MatchG" | "MemG", _) => {
            Err(DecodeError::Expected("valid SL guard arity"))
        }
        (unknown, _) => Err(DecodeError::UnknownVariant(unknown.to_owned())),
    }
}

fn encode_guard(guard: &Guard) -> Value {
    match guard {
        Guard::Bool(value) => json!(["BoolG", value]),
        Guard::Cmp(op, typ, exp) => json!([
            "CmpG",
            il::encode_cmp_op(*op),
            il::encode_op_typ(*typ),
            il::encode_exp(exp)
        ]),
        Guard::Sub(typ, subcheck) => {
            json!(["SubG", il::encode_typ(typ), il::encode_subcheck(subcheck)])
        }
        Guard::Match(pattern) => json!(["MatchG", il::encode_pattern(pattern)]),
        Guard::Mem(exp) => json!(["MemG", il::encode_exp(exp)]),
    }
}

fn decode_iid(value: &Value) -> Result<ast::Iid, DecodeError> {
    let object = object(value)?;
    let iid = integer(field(object, "iid")?)?;
    Ok(ast::Iid::new(iid))
}

fn encode_iid(iid: &ast::Iid) -> Value {
    json!({"iid": iid.get()})
}

fn decode_instr(value: &Value) -> Result<ast::Instr, DecodeError> {
    source::decode_note_phrase(value, decode_instr_kind, decode_iid)
}

fn encode_instr(instr: &ast::Instr) -> Value {
    source::encode_note_phrase(instr, encode_instr_kind, encode_iid)
}

fn decode_instr_kind(value: &Value) -> Result<InstrKind, DecodeError> {
    let (tag, fields) = variant(value)?;
    match (tag, fields) {
        ("IfI", [exp, iters, block, dangle]) => Ok(InstrKind::If(IfInstr {
            exp: il::decode_exp(exp)?,
            iter_exps: il::decode_list(iters, il::decode_iter_exp)?,
            block: decode_block(block)?,
            dangle: boolean(dangle)?,
        })),
        ("HoldI", [id, exp, iters, case]) => Ok(InstrKind::Hold(HoldInstr {
            id: il::decode_id(id)?,
            not_exp: il::decode_not_exp(exp)?,
            iter_exps: il::decode_list(iters, il::decode_iter_exp)?,
            hold_case: decode_hold_case(case)?,
        })),
        ("CaseI", [exp, cases, dangle]) => Ok(InstrKind::Case(CaseInstr {
            exp: il::decode_exp(exp)?,
            cases: il::decode_list(cases, decode_case)?,
            dangle: boolean(dangle)?,
        })),
        ("GroupI", [id, rel_signature, exps, block]) => Ok(InstrKind::Group(GroupInstr {
            id: il::decode_id(id)?,
            rel_signature: decode_rel_signature(rel_signature)?,
            exps: il::decode_list(exps, il::decode_exp)?,
            block: decode_block(block)?,
        })),
        ("LetI", [exp_l, exp_r, iters, block]) => Ok(InstrKind::Let(LetInstr {
            exp_l: il::decode_exp(exp_l)?,
            exp_r: il::decode_exp(exp_r)?,
            iter_instrs: il::decode_list(iters, il::decode_prem_iter)?,
            block: decode_block(block)?,
        })),
        ("RuleI", [id, exp, input, iters, block]) => Ok(InstrKind::Rule(RuleInstr {
            id: il::decode_id(id)?,
            not_exp: il::decode_not_exp(exp)?,
            input_hint: il::decode_input_hint(input)?,
            iter_instrs: il::decode_list(iters, il::decode_prem_iter)?,
            block: decode_block(block)?,
        })),
        ("ResultI", [rel_signature, exps]) => Ok(InstrKind::Result(ResultInstr {
            rel_signature: decode_rel_signature(rel_signature)?,
            exps: il::decode_list(exps, il::decode_exp)?,
        })),
        ("ReturnI", [exp]) => Ok(InstrKind::Return(ReturnInstr {
            exp: il::decode_exp(exp)?,
        })),
        ("DebugI", [exp, instr]) => Ok(InstrKind::Debug(DebugInstr {
            exp: il::decode_exp(exp)?,
            instr: Box::new(decode_instr(instr)?),
        })),
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
        InstrKind::If(IfInstr {
            exp,
            iter_exps: iters,
            block,
            dangle,
        }) => json!([
            "IfI",
            il::encode_exp(exp),
            il::encode_list(iters, il::encode_iter_exp),
            encode_block(block),
            dangle
        ]),
        InstrKind::Hold(HoldInstr {
            id,
            not_exp,
            iter_exps: iters,
            hold_case: case,
        }) => json!([
            "HoldI",
            il::encode_id(id),
            il::encode_not_exp(not_exp),
            il::encode_list(iters, il::encode_iter_exp),
            encode_hold_case(case)
        ]),
        InstrKind::Case(CaseInstr { exp, cases, dangle }) => json!([
            "CaseI",
            il::encode_exp(exp),
            il::encode_list(cases, encode_case),
            dangle
        ]),
        InstrKind::Group(GroupInstr {
            id,
            rel_signature,
            exps,
            block,
        }) => json!([
            "GroupI",
            il::encode_id(id),
            encode_rel_signature(rel_signature),
            il::encode_list(exps, il::encode_exp),
            encode_block(block)
        ]),
        InstrKind::Let(LetInstr {
            exp_l,
            exp_r,
            iter_instrs: iters,
            block,
        }) => json!([
            "LetI",
            il::encode_exp(exp_l),
            il::encode_exp(exp_r),
            il::encode_list(iters, il::encode_prem_iter),
            encode_block(block)
        ]),
        InstrKind::Rule(RuleInstr {
            id,
            not_exp,
            input_hint: input,
            iter_instrs: iters,
            block,
        }) => json!([
            "RuleI",
            il::encode_id(id),
            il::encode_not_exp(not_exp),
            il::encode_input_hint(input),
            il::encode_list(iters, il::encode_prem_iter),
            encode_block(block)
        ]),
        InstrKind::Result(ResultInstr {
            rel_signature,
            exps,
        }) => json!([
            "ResultI",
            encode_rel_signature(rel_signature),
            il::encode_list(exps, il::encode_exp)
        ]),
        InstrKind::Return(ReturnInstr { exp }) => json!(["ReturnI", il::encode_exp(exp)]),
        InstrKind::Debug(DebugInstr { exp, instr }) => {
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
        [typ, input] => Ok(ast::RelSignature {
            not_typ: il::decode_not_typ(typ)?,
            input_hint: il::decode_input_hint(input)?,
        }),
        _ => Err(DecodeError::Expected("SL relation signature pair")),
    }
}

fn encode_rel_signature(rel_signature: &ast::RelSignature) -> Value {
    json!([
        il::encode_not_typ(&rel_signature.not_typ),
        il::encode_input_hint(&rel_signature.input_hint)
    ])
}

fn decode_extern_rel(value: &Value) -> Result<ast::ExternRel, DecodeError> {
    match array(value)? {
        [id, rel_signature, exps_input, hints] => Ok(ast::ExternRel {
            id: il::decode_id(id)?,
            rel_signature: decode_rel_signature(rel_signature)?,
            exps_input: il::decode_list(exps_input, il::decode_exp)?,
            hints: il::decode_list(hints, el::decode_hint)?,
        }),
        _ => Err(DecodeError::Expected("SL external relation tuple")),
    }
}

fn encode_extern_rel(relation: &ast::ExternRel) -> Value {
    json!([
        il::encode_id(&relation.id),
        encode_rel_signature(&relation.rel_signature),
        il::encode_list(&relation.exps_input, il::encode_exp),
        il::encode_list(&relation.hints, el::encode_hint)
    ])
}

fn decode_rel(value: &Value) -> Result<ast::Rel, DecodeError> {
    match array(value)? {
        [id, rel_signature, exps_input, block, else_block, hints] => Ok(ast::Rel {
            id: il::decode_id(id)?,
            rel_signature: decode_rel_signature(rel_signature)?,
            exps_input: il::decode_list(exps_input, il::decode_exp)?,
            block: decode_block(block)?,
            else_block: decode_option(else_block, decode_block)?,
            hints: il::decode_list(hints, el::decode_hint)?,
        }),
        _ => Err(DecodeError::Expected("SL relation tuple")),
    }
}

fn encode_rel(relation: &ast::Rel) -> Value {
    json!([
        il::encode_id(&relation.id),
        encode_rel_signature(&relation.rel_signature),
        il::encode_list(&relation.exps_input, il::encode_exp),
        encode_block(&relation.block),
        encode_option(relation.else_block.as_ref(), encode_block),
        il::encode_list(&relation.hints, el::encode_hint)
    ])
}

fn decode_extern_func(value: &Value) -> Result<ast::ExternFunc, DecodeError> {
    match array(value)? {
        [id, tparams, params, typ, hints] => Ok(ast::ExternFunc {
            id: il::decode_id(id)?,
            tparams: il::decode_list(tparams, il::decode_tparam)?,
            params: il::decode_list(params, decode_param)?,
            typ: il::decode_typ(typ)?,
            hints: il::decode_list(hints, el::decode_hint)?,
        }),
        _ => Err(DecodeError::Expected("SL external function tuple")),
    }
}

fn encode_extern_func(function: &ast::ExternFunc) -> Value {
    json!([
        il::encode_id(&function.id),
        il::encode_list(&function.tparams, il::encode_tparam),
        il::encode_list(&function.params, encode_param),
        il::encode_typ(&function.typ),
        il::encode_list(&function.hints, el::encode_hint)
    ])
}

fn encode_builtin_func(function: &ast::BuiltinFunc) -> Value {
    json!([
        il::encode_id(&function.id),
        il::encode_list(&function.tparams, il::encode_tparam),
        il::encode_list(&function.params, encode_param),
        il::encode_typ(&function.typ),
        il::encode_list(&function.hints, el::encode_hint)
    ])
}

fn decode_table_row(value: &Value) -> Result<ast::TableRow, DecodeError> {
    match array(value)? {
        [exps_input, exp, block] => Ok(ast::TableRow {
            exps_input: il::decode_list(exps_input, il::decode_exp)?,
            exp: il::decode_exp(exp)?,
            block: decode_block(block)?,
        }),
        _ => Err(DecodeError::Expected("SL table row triple")),
    }
}

fn encode_table_row(row: &ast::TableRow) -> Value {
    json!([
        il::encode_list(&row.exps_input, il::encode_exp),
        il::encode_exp(&row.exp),
        encode_block(&row.block)
    ])
}

fn decode_table_func(value: &Value) -> Result<ast::TableFunc, DecodeError> {
    match array(value)? {
        [id, params, typ, table_rows, hints] => Ok(ast::TableFunc {
            id: il::decode_id(id)?,
            params: il::decode_list(params, decode_param)?,
            typ: il::decode_typ(typ)?,
            table_rows: il::decode_list(table_rows, decode_table_row)?,
            hints: il::decode_list(hints, el::decode_hint)?,
        }),
        _ => Err(DecodeError::Expected("SL table function tuple")),
    }
}

fn encode_table_func(function: &ast::TableFunc) -> Value {
    json!([
        il::encode_id(&function.id),
        il::encode_list(&function.params, encode_param),
        il::encode_typ(&function.typ),
        il::encode_list(&function.table_rows, encode_table_row),
        il::encode_list(&function.hints, el::encode_hint)
    ])
}

fn decode_defined_func(value: &Value) -> Result<ast::DefinedFunc, DecodeError> {
    match array(value)? {
        [id, tparams, params, typ, block, else_block, hints] => Ok(ast::DefinedFunc {
            id: il::decode_id(id)?,
            tparams: il::decode_list(tparams, il::decode_tparam)?,
            params: il::decode_list(params, decode_param)?,
            typ: il::decode_typ(typ)?,
            block: decode_block(block)?,
            else_block: decode_option(else_block, decode_block)?,
            hints: il::decode_list(hints, el::decode_hint)?,
        }),
        _ => Err(DecodeError::Expected("SL defined function tuple")),
    }
}

fn encode_defined_func(function: &ast::DefinedFunc) -> Value {
    json!([
        il::encode_id(&function.id),
        il::encode_list(&function.tparams, il::encode_tparam),
        il::encode_list(&function.params, encode_param),
        il::encode_typ(&function.typ),
        encode_block(&function.block),
        encode_option(function.else_block.as_ref(), encode_block),
        il::encode_list(&function.hints, el::encode_hint)
    ])
}

fn decode_def(value: &Value) -> Result<ast::Def, DecodeError> {
    source::decode_phrase(value, |value| {
        let (tag, fields) = variant(value)?;
        match (tag, fields) {
            ("ExternTypD", [id, hints]) => Ok(DefKind::ExternTyp(ExternTypDef {
                id: il::decode_id(id)?,
                hints: il::decode_list(hints, el::decode_hint)?,
            })),
            ("TypD", [id, tparams, typ, hints]) => Ok(DefKind::Typ(TypDef {
                id: il::decode_id(id)?,
                tparams: il::decode_list(tparams, il::decode_tparam)?,
                def_typ: super::il::decode_def_typ(typ)?,
                hints: il::decode_list(hints, el::decode_hint)?,
            })),
            ("VarD", [id, typ, hints]) => Ok(DefKind::Var(VarDef {
                id: il::decode_id(id)?,
                typ: il::decode_typ(typ)?,
                hints: il::decode_list(hints, el::decode_hint)?,
            })),
            ("ExternRelD", [rel]) => Ok(DefKind::ExternRel(decode_extern_rel(rel)?)),
            ("RelD", [rel]) => Ok(DefKind::Rel(decode_rel(rel)?)),
            ("ExternDecD", [func]) => Ok(DefKind::ExternDec(decode_extern_func(func)?)),
            ("BuiltinDecD", [func]) => Ok(DefKind::BuiltinDec({
                let function = decode_extern_func(func)?;
                ast::BuiltinFunc {
                    id: function.id,
                    tparams: function.tparams,
                    params: function.params,
                    typ: function.typ,
                    hints: function.hints,
                }
            })),
            ("TableDecD", [func]) => Ok(DefKind::TableDec(decode_table_func(func)?)),
            ("FuncDecD", [func]) => Ok(DefKind::FuncDec(decode_defined_func(func)?)),
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
        DefKind::ExternTyp(ExternTypDef { id, hints }) => json!([
            "ExternTypD",
            il::encode_id(id),
            il::encode_list(hints, el::encode_hint)
        ]),
        DefKind::Typ(TypDef {
            id,
            tparams,
            def_typ: typ,
            hints,
        }) => json!([
            "TypD",
            il::encode_id(id),
            il::encode_list(tparams, il::encode_tparam),
            super::il::encode_def_typ(typ),
            il::encode_list(hints, el::encode_hint)
        ]),
        DefKind::Var(VarDef { id, typ, hints }) => json!([
            "VarD",
            il::encode_id(id),
            il::encode_typ(typ),
            il::encode_list(hints, el::encode_hint)
        ]),
        DefKind::ExternRel(rel) => json!(["ExternRelD", encode_extern_rel(rel)]),
        DefKind::Rel(rel) => json!(["RelD", encode_rel(rel)]),
        DefKind::ExternDec(func) => json!(["ExternDecD", encode_extern_func(func)]),
        DefKind::BuiltinDec(func) => json!(["BuiltinDecD", encode_builtin_func(func)]),
        DefKind::TableDec(func) => json!(["TableDecD", encode_table_func(func)]),
        DefKind::FuncDec(func) => json!(["FuncDecD", encode_defined_func(func)]),
    })
}
