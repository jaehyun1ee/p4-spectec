use serde_json::{Value, json};

use crate::lang::al::ast::{self, *};

use super::{
    super::{
        DecodeError, EncodeError, array, decode_list, decode_option, encode_list, encode_option,
        on_codec_stack, variant,
    },
    el, il,
};
use crate::wire::ocaml::source;

pub struct SpecCodec;

impl SpecCodec {
    pub fn decode(value: &Value) -> Result<ast::Spec, DecodeError> {
        on_codec_stack(|| decode_list(value, decode_def))
    }

    pub fn encode(spec: &ast::Spec) -> Result<Value, EncodeError> {
        on_codec_stack(|| Ok(encode_list(spec, encode_def)))
    }
}

fn decode_rule_match(value: &Value) -> Result<ast::RuleMatch, DecodeError> {
    match array(value)? {
        [exps_signature, exps_input, prems] => Ok(ast::RuleMatch {
            exps_signature: decode_list(exps_signature, il::decode_exp)?,
            exps_input: decode_list(exps_input, il::decode_exp)?,
            prems: decode_list(prems, il::decode_prem)?,
        }),
        _ => Err(DecodeError::Expected("AL rule match triple")),
    }
}

fn encode_rule_match(rule_match: &ast::RuleMatch) -> Value {
    json!([
        encode_list(&rule_match.exps_signature, il::encode_exp),
        encode_list(&rule_match.exps_input, il::encode_exp),
        encode_list(&rule_match.prems, il::encode_prem)
    ])
}

fn decode_rule_path(value: &Value) -> Result<ast::RulePath, DecodeError> {
    match array(value)? {
        [id, prems, exps_output] => Ok(ast::RulePath {
            id: il::decode_id(id)?,
            prems: decode_list(prems, il::decode_prem)?,
            exps_output: decode_list(exps_output, il::decode_exp)?,
        }),
        _ => Err(DecodeError::Expected("AL rule path triple")),
    }
}

fn encode_rule_path(rule_path: &ast::RulePath) -> Value {
    json!([
        il::encode_id(&rule_path.id),
        encode_list(&rule_path.prems, il::encode_prem),
        encode_list(&rule_path.exps_output, il::encode_exp)
    ])
}

fn decode_rule_group(value: &Value) -> Result<ast::RuleGroup, DecodeError> {
    source::decode_phrase(value, |value| match array(value)? {
        [id, rule_match, rule_paths] => Ok(ast::RuleGroupKind {
            id: il::decode_id(id)?,
            rule_match: decode_rule_match(rule_match)?,
            rule_paths: decode_list(rule_paths, decode_rule_path)?,
        }),
        _ => Err(DecodeError::Expected("AL rule group triple")),
    })
}

fn encode_rule_group(group: &ast::RuleGroup) -> Value {
    source::encode_phrase(group, |group| {
        json!([
            il::encode_id(&group.id),
            encode_rule_match(&group.rule_match),
            encode_list(&group.rule_paths, encode_rule_path)
        ])
    })
}

fn decode_else_group(value: &Value) -> Result<ast::ElseGroup, DecodeError> {
    source::decode_phrase(value, |value| match array(value)? {
        [id, rule_match, rule_path] => Ok(ast::ElseGroupKind {
            id: il::decode_id(id)?,
            rule_match: decode_rule_match(rule_match)?,
            rule_path: decode_rule_path(rule_path)?,
        }),
        _ => Err(DecodeError::Expected("AL else group triple")),
    })
}

fn encode_else_group(group: &ast::ElseGroup) -> Value {
    source::encode_phrase(group, |group| {
        json!([
            il::encode_id(&group.id),
            encode_rule_match(&group.rule_match),
            encode_rule_path(&group.rule_path)
        ])
    })
}

fn decode_table_row(value: &Value) -> Result<ast::TableRow, DecodeError> {
    source::decode_phrase(value, |value| match array(value)? {
        [exps_signature, args, exp, prems] => Ok(ast::TableRowKind {
            exps_signature: decode_list(exps_signature, il::decode_exp)?,
            args: decode_list(args, il::decode_arg)?,
            exp: il::decode_exp(exp)?,
            prems: decode_list(prems, il::decode_prem)?,
        }),
        _ => Err(DecodeError::Expected("AL table row quadruple")),
    })
}

fn encode_table_row(row: &ast::TableRow) -> Value {
    source::encode_phrase(row, |row| {
        json!([
            encode_list(&row.exps_signature, il::encode_exp),
            encode_list(&row.args, il::encode_arg),
            il::encode_exp(&row.exp),
            encode_list(&row.prems, il::encode_prem)
        ])
    })
}

fn decode_def(value: &Value) -> Result<ast::Def, DecodeError> {
    source::decode_phrase(value, |value| {
        let (tag, fields) = variant(value)?;
        match (tag, fields) {
            ("ExternTypD", [id, hints]) => Ok(DefKind::ExternTyp(ExternTypDef {
                id: il::decode_id(id)?,
                hints: decode_list(hints, el::decode_hint)?,
            })),
            ("TypD", [id, tparams, typ, hints]) => Ok(DefKind::Typ(TypDef {
                id: il::decode_id(id)?,
                tparams: decode_list(tparams, il::decode_tparam)?,
                def_typ: il::decode_def_typ(typ)?,
                hints: decode_list(hints, el::decode_hint)?,
            })),
            ("VarD", [id, typ, hints]) => Ok(DefKind::Var(VarDef {
                id: il::decode_id(id)?,
                typ: il::decode_typ(typ)?,
                hints: decode_list(hints, el::decode_hint)?,
            })),
            ("ExternRelD", [id, typ, input, hints]) => Ok(DefKind::ExternRel(ExternRelDef {
                id: il::decode_id(id)?,
                not_typ: il::decode_not_typ(typ)?,
                input_hint: il::decode_input_hint(input)?,
                hints: decode_list(hints, el::decode_hint)?,
            })),
            ("RelD", [id, typ, input, groups, else_group, hints]) => Ok(DefKind::Rel(RelDef {
                id: il::decode_id(id)?,
                not_typ: il::decode_not_typ(typ)?,
                input_hint: il::decode_input_hint(input)?,
                rule_groups: decode_list(groups, decode_rule_group)?,
                else_group: decode_option(else_group, decode_else_group)?,
                hints: decode_list(hints, el::decode_hint)?,
            })),
            ("ExternDecD", [id, tparams, params, typ, hints]) => {
                Ok(DefKind::ExternDec(ExternDecDef {
                    id: il::decode_id(id)?,
                    tparams: decode_list(tparams, il::decode_tparam)?,
                    params: decode_list(params, il::decode_param)?,
                    typ: il::decode_typ(typ)?,
                    hints: decode_list(hints, el::decode_hint)?,
                }))
            }
            ("BuiltinDecD", [id, tparams, params, typ, hints]) => {
                Ok(DefKind::BuiltinDec(BuiltinDecDef {
                    id: il::decode_id(id)?,
                    tparams: decode_list(tparams, il::decode_tparam)?,
                    params: decode_list(params, il::decode_param)?,
                    typ: il::decode_typ(typ)?,
                    hints: decode_list(hints, el::decode_hint)?,
                }))
            }
            ("TableDecD", [id, params, typ, table_rows, hints]) => {
                Ok(DefKind::TableDec(TableDecDef {
                    id: il::decode_id(id)?,
                    params: decode_list(params, il::decode_param)?,
                    typ: il::decode_typ(typ)?,
                    table_rows: decode_list(table_rows, decode_table_row)?,
                    hints: decode_list(hints, el::decode_hint)?,
                }))
            }
            ("FuncDecD", [id, tparams, params, typ, clauses, else_clause, hints]) => {
                Ok(DefKind::FuncDec(FuncDecDef {
                    id: il::decode_id(id)?,
                    tparams: decode_list(tparams, il::decode_tparam)?,
                    params: decode_list(params, il::decode_param)?,
                    typ: il::decode_typ(typ)?,
                    clauses: decode_list(clauses, il::decode_clause)?,
                    else_clause: decode_option(else_clause, il::decode_clause)?,
                    hints: decode_list(hints, el::decode_hint)?,
                }))
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
        DefKind::ExternTyp(ExternTypDef { id, hints }) => json!([
            "ExternTypD",
            il::encode_id(id),
            encode_list(hints, el::encode_hint)
        ]),
        DefKind::Typ(TypDef {
            id,
            tparams,
            def_typ: typ,
            hints,
        }) => json!([
            "TypD",
            il::encode_id(id),
            encode_list(tparams, il::encode_tparam),
            il::encode_def_typ(typ),
            encode_list(hints, el::encode_hint)
        ]),
        DefKind::Var(VarDef { id, typ, hints }) => json!([
            "VarD",
            il::encode_id(id),
            il::encode_typ(typ),
            encode_list(hints, el::encode_hint)
        ]),
        DefKind::ExternRel(ExternRelDef {
            id,
            not_typ: typ,
            input_hint: input,
            hints,
        }) => json!([
            "ExternRelD",
            il::encode_id(id),
            il::encode_not_typ(typ),
            il::encode_input_hint(input),
            encode_list(hints, el::encode_hint)
        ]),
        DefKind::Rel(RelDef {
            id,
            not_typ: typ,
            input_hint: input,
            rule_groups: groups,
            else_group,
            hints,
        }) => json!([
            "RelD",
            il::encode_id(id),
            il::encode_not_typ(typ),
            il::encode_input_hint(input),
            encode_list(groups, encode_rule_group),
            encode_option(else_group.as_ref(), encode_else_group),
            encode_list(hints, el::encode_hint)
        ]),
        DefKind::ExternDec(ExternDecDef {
            id,
            tparams,
            params,
            typ,
            hints,
        }) => json!([
            "ExternDecD",
            il::encode_id(id),
            encode_list(tparams, il::encode_tparam),
            encode_list(params, il::encode_param),
            il::encode_typ(typ),
            encode_list(hints, el::encode_hint)
        ]),
        DefKind::BuiltinDec(BuiltinDecDef {
            id,
            tparams,
            params,
            typ,
            hints,
        }) => json!([
            "BuiltinDecD",
            il::encode_id(id),
            encode_list(tparams, il::encode_tparam),
            encode_list(params, il::encode_param),
            il::encode_typ(typ),
            encode_list(hints, el::encode_hint)
        ]),
        DefKind::TableDec(TableDecDef {
            id,
            params,
            typ,
            table_rows,
            hints,
        }) => json!([
            "TableDecD",
            il::encode_id(id),
            encode_list(params, il::encode_param),
            il::encode_typ(typ),
            encode_list(table_rows, encode_table_row),
            encode_list(hints, el::encode_hint)
        ]),
        DefKind::FuncDec(FuncDecDef {
            id,
            tparams,
            params,
            typ,
            clauses,
            else_clause,
            hints,
        }) => json!([
            "FuncDecD",
            il::encode_id(id),
            encode_list(tparams, il::encode_tparam),
            encode_list(params, il::encode_param),
            il::encode_typ(typ),
            encode_list(clauses, il::encode_clause),
            encode_option(else_clause.as_ref(), il::encode_clause),
            encode_list(hints, el::encode_hint)
        ]),
    })
}
