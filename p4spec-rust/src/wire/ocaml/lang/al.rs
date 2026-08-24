use serde_json::{Value, json};

use crate::lang::al::ast::{self, DefKind};

use super::{
    super::{DecodeError, EncodeError, array, on_codec_stack, variant},
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

fn decode_rule_match(value: &Value) -> Result<ast::RuleMatch, DecodeError> {
    match array(value)? {
        [inputs, outputs, prems] => Ok((
            il::decode_list(inputs, il::decode_exp)?,
            il::decode_list(outputs, il::decode_exp)?,
            il::decode_list(prems, il::decode_prem)?,
        )),
        _ => Err(DecodeError::Expected("AL rule match triple")),
    }
}

fn encode_rule_match((inputs, outputs, prems): &ast::RuleMatch) -> Value {
    json!([
        il::encode_list(inputs, il::encode_exp),
        il::encode_list(outputs, il::encode_exp),
        il::encode_list(prems, il::encode_prem)
    ])
}

fn decode_rule_path(value: &Value) -> Result<ast::RulePath, DecodeError> {
    match array(value)? {
        [id, prems, exps] => Ok((
            il::decode_id(id)?,
            il::decode_list(prems, il::decode_prem)?,
            il::decode_list(exps, il::decode_exp)?,
        )),
        _ => Err(DecodeError::Expected("AL rule path triple")),
    }
}

fn encode_rule_path((id, prems, exps): &ast::RulePath) -> Value {
    json!([
        il::encode_id(id),
        il::encode_list(prems, il::encode_prem),
        il::encode_list(exps, il::encode_exp)
    ])
}

fn decode_rule_group(value: &Value) -> Result<ast::RuleGroup, DecodeError> {
    source::decode_phrase(value, |value| match array(value)? {
        [id, rule_match, paths] => Ok((
            il::decode_id(id)?,
            decode_rule_match(rule_match)?,
            il::decode_list(paths, decode_rule_path)?,
        )),
        _ => Err(DecodeError::Expected("AL rule group triple")),
    })
}

fn encode_rule_group(group: &ast::RuleGroup) -> Value {
    source::encode_phrase(group, |(id, rule_match, paths)| {
        json!([
            il::encode_id(id),
            encode_rule_match(rule_match),
            il::encode_list(paths, encode_rule_path)
        ])
    })
}

fn decode_else_group(value: &Value) -> Result<ast::ElseGroup, DecodeError> {
    source::decode_phrase(value, |value| match array(value)? {
        [id, rule_match, path] => Ok((
            il::decode_id(id)?,
            decode_rule_match(rule_match)?,
            decode_rule_path(path)?,
        )),
        _ => Err(DecodeError::Expected("AL else group triple")),
    })
}

fn encode_else_group(group: &ast::ElseGroup) -> Value {
    source::encode_phrase(group, |(id, rule_match, path)| {
        json!([
            il::encode_id(id),
            encode_rule_match(rule_match),
            encode_rule_path(path)
        ])
    })
}

fn decode_table_row(value: &Value) -> Result<ast::TableRow, DecodeError> {
    source::decode_phrase(value, |value| match array(value)? {
        [inputs, args, exp, prems] => Ok((
            il::decode_list(inputs, il::decode_exp)?,
            il::decode_list(args, il::decode_arg)?,
            il::decode_exp(exp)?,
            il::decode_list(prems, il::decode_prem)?,
        )),
        _ => Err(DecodeError::Expected("AL table row quadruple")),
    })
}

fn encode_table_row(row: &ast::TableRow) -> Value {
    source::encode_phrase(row, |(inputs, args, exp, prems)| {
        json!([
            il::encode_list(inputs, il::encode_exp),
            il::encode_list(args, il::encode_arg),
            il::encode_exp(exp),
            il::encode_list(prems, il::encode_prem)
        ])
    })
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
                il::decode_def_typ(typ)?,
                il::decode_list(hints, el::decode_hint)?,
            )),
            ("VarD", [id, typ, hints]) => Ok(DefKind::VarD(
                il::decode_id(id)?,
                il::decode_typ(typ)?,
                il::decode_list(hints, el::decode_hint)?,
            )),
            ("ExternRelD", [id, typ, input, hints]) => Ok(DefKind::ExternRelD(
                il::decode_id(id)?,
                il::decode_not_typ(typ)?,
                il::decode_input_hint(input)?,
                il::decode_list(hints, el::decode_hint)?,
            )),
            ("RelD", [id, typ, input, groups, else_group, hints]) => Ok(DefKind::RelD(
                il::decode_id(id)?,
                il::decode_not_typ(typ)?,
                il::decode_input_hint(input)?,
                il::decode_list(groups, decode_rule_group)?,
                il::decode_option(else_group, decode_else_group)?,
                il::decode_list(hints, el::decode_hint)?,
            )),
            ("ExternDecD", [id, tparams, params, typ, hints]) => Ok(DefKind::ExternDecD(
                il::decode_id(id)?,
                il::decode_list(tparams, il::decode_tparam)?,
                il::decode_list(params, il::decode_param)?,
                il::decode_typ(typ)?,
                il::decode_list(hints, el::decode_hint)?,
            )),
            ("BuiltinDecD", [id, tparams, params, typ, hints]) => Ok(DefKind::BuiltinDecD(
                il::decode_id(id)?,
                il::decode_list(tparams, il::decode_tparam)?,
                il::decode_list(params, il::decode_param)?,
                il::decode_typ(typ)?,
                il::decode_list(hints, el::decode_hint)?,
            )),
            ("TableDecD", [id, params, typ, rows, hints]) => Ok(DefKind::TableDecD(
                il::decode_id(id)?,
                il::decode_list(params, il::decode_param)?,
                il::decode_typ(typ)?,
                il::decode_list(rows, decode_table_row)?,
                il::decode_list(hints, el::decode_hint)?,
            )),
            ("FuncDecD", [id, tparams, params, typ, clauses, else_clause, hints]) => {
                Ok(DefKind::FuncDecD(
                    il::decode_id(id)?,
                    il::decode_list(tparams, il::decode_tparam)?,
                    il::decode_list(params, il::decode_param)?,
                    il::decode_typ(typ)?,
                    il::decode_list(clauses, il::decode_clause)?,
                    il::decode_option(else_clause, il::decode_clause)?,
                    il::decode_list(hints, el::decode_hint)?,
                ))
            }
            (
                "ExternTypD" | "TypD" | "VarD" | "ExternRelD" | "RelD" | "ExternDecD"
                | "BuiltinDecD" | "TableDecD" | "FuncDecD",
                _,
            ) => Err(DecodeError::Expected("valid AL definition arity")),
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
            il::encode_def_typ(typ),
            il::encode_list(hints, el::encode_hint)
        ]),
        DefKind::VarD(id, typ, hints) => json!([
            "VarD",
            il::encode_id(id),
            il::encode_typ(typ),
            il::encode_list(hints, el::encode_hint)
        ]),
        DefKind::ExternRelD(id, typ, input, hints) => json!([
            "ExternRelD",
            il::encode_id(id),
            il::encode_not_typ(typ),
            il::encode_input_hint(input),
            il::encode_list(hints, el::encode_hint)
        ]),
        DefKind::RelD(id, typ, input, groups, else_group, hints) => json!([
            "RelD",
            il::encode_id(id),
            il::encode_not_typ(typ),
            il::encode_input_hint(input),
            il::encode_list(groups, encode_rule_group),
            il::encode_option(else_group.as_ref(), encode_else_group),
            il::encode_list(hints, el::encode_hint)
        ]),
        DefKind::ExternDecD(id, tparams, params, typ, hints) => json!([
            "ExternDecD",
            il::encode_id(id),
            il::encode_list(tparams, il::encode_tparam),
            il::encode_list(params, il::encode_param),
            il::encode_typ(typ),
            il::encode_list(hints, el::encode_hint)
        ]),
        DefKind::BuiltinDecD(id, tparams, params, typ, hints) => json!([
            "BuiltinDecD",
            il::encode_id(id),
            il::encode_list(tparams, il::encode_tparam),
            il::encode_list(params, il::encode_param),
            il::encode_typ(typ),
            il::encode_list(hints, el::encode_hint)
        ]),
        DefKind::TableDecD(id, params, typ, rows, hints) => json!([
            "TableDecD",
            il::encode_id(id),
            il::encode_list(params, il::encode_param),
            il::encode_typ(typ),
            il::encode_list(rows, encode_table_row),
            il::encode_list(hints, el::encode_hint)
        ]),
        DefKind::FuncDecD(id, tparams, params, typ, clauses, else_clause, hints) => json!([
            "FuncDecD",
            il::encode_id(id),
            il::encode_list(tparams, il::encode_tparam),
            il::encode_list(params, il::encode_param),
            il::encode_typ(typ),
            il::encode_list(clauses, il::encode_clause),
            il::encode_option(else_clause.as_ref(), il::encode_clause),
            il::encode_list(hints, el::encode_hint)
        ]),
    })
}
