//! Convert algorithmic definitions through ordered blocks to structured syntax

use super::{
    StructureError, StructureErrorKind,
    antiunify::{self, ClausePath},
    context::Context,
    dangle, merge,
    ol::ast as ol,
    optimize, prettify, totalize,
};
use crate::lang::{
    al::{ast as al, fresh},
    common::{
        ds::set::IdSet,
        source::{Phrase, Span},
    },
    hints::input,
    sl::ast as sl,
    traits::{eq::SyntaxEq, free::Free},
};

// Parameters

fn struct_param(ctx: &Context, frees: &mut IdSet, param_al: al::Param) -> sl::Param {
    let Phrase {
        node: param_kind_al,
        span,
        ..
    } = param_al;
    let param_kind_sl = struct_param_kind(ctx, frees, param_kind_al);
    crate::phrase! {node: param_kind_sl, span: span}
}

fn struct_param_kind(
    ctx: &Context,
    frees: &mut IdSet,
    param_kind_al: al::ParamKind,
) -> sl::ParamKind {
    match param_kind_al {
        al::ParamKind::Exp(typ) => struct_exp_param(ctx, frees, typ),
        al::ParamKind::Def(id, tparams, params_al, typ) => {
            struct_def_param(ctx, id, tparams, params_al, typ)
        }
    }
}

fn struct_exp_param(ctx: &Context, frees: &mut IdSet, typ: al::Typ) -> sl::ParamKind {
    let (frees_next, exp_input) = fresh::exp_from_typ(true, &ctx.menv, frees, &typ);
    *frees = frees_next;
    sl::ParamKind::Exp(typ, Box::new(exp_input))
}

fn struct_def_param(
    ctx: &Context,
    id: al::Id,
    tparams: Vec<al::TParam>,
    params_al: Vec<al::Param>,
    typ: al::Typ,
) -> sl::ParamKind {
    let params_sl = struct_params(ctx, params_al);
    sl::ParamKind::Def(id, tparams, params_sl, typ)
}

fn struct_params(ctx: &Context, params_al: Vec<al::Param>) -> Vec<sl::Param> {
    let mut frees = IdSet::new();
    params_al
        .into_iter()
        .map(|param_al| struct_param(ctx, &mut frees, param_al))
        .collect()
}

fn struct_params_from_args(
    ctx: &Context,
    params_al: Vec<al::Param>,
    args_input: Vec<al::Arg>,
    span: &Span,
) -> Result<Vec<sl::Param>, StructureError> {
    check_arity(params_al.len(), args_input.len(), span)?;
    params_al
        .into_iter()
        .zip(args_input)
        .map(|(param_al, arg_input)| struct_param_from_arg(ctx, param_al, arg_input))
        .collect()
}

fn struct_param_from_arg(
    ctx: &Context,
    param_al: al::Param,
    arg_input: al::Arg,
) -> Result<sl::Param, StructureError> {
    let Phrase {
        node: param_kind_al,
        span,
        ..
    } = param_al;
    let Phrase { node: arg_kind, .. } = arg_input;
    let param_kind_sl = struct_param_kind_from_arg(ctx, param_kind_al, arg_kind, &span)?;
    Ok(crate::phrase! {node: param_kind_sl, span: span})
}

fn struct_param_kind_from_arg(
    ctx: &Context,
    param_kind_al: al::ParamKind,
    arg_kind: al::ArgKind,
    span: &Span,
) -> Result<sl::ParamKind, StructureError> {
    match (param_kind_al, arg_kind) {
        (al::ParamKind::Exp(typ), al::ArgKind::Exp(exp)) => Ok(sl::ParamKind::Exp(typ, exp)),
        (al::ParamKind::Def(id, tparams, params_al, typ), al::ArgKind::Def(id_arg)) => {
            struct_def_param_from_arg(ctx, id, tparams, params_al, typ, id_arg, span)
        }
        _ => Err(StructureError::new(
            StructureErrorKind::IncompatibleParameterArgument,
            span.clone(),
        )),
    }
}

fn struct_def_param_from_arg(
    ctx: &Context,
    id: al::Id,
    tparams: Vec<al::TParam>,
    params_al: Vec<al::Param>,
    typ: al::Typ,
    id_arg: al::Id,
    span: &Span,
) -> Result<sl::ParamKind, StructureError> {
    if !id.syntax_eq(&id_arg) {
        return Err(StructureError::new(
            StructureErrorKind::IncompatibleParameterArgument,
            span.clone(),
        ));
    }
    Ok(struct_def_param(ctx, id, tparams, params_al, typ))
}

// Premises

fn internalize_iter(
    prem_al: al::Prem,
    iter_prems: Vec<al::PremIter>,
) -> (al::Prem, Vec<al::PremIter>) {
    let Phrase {
        node: prem_kind_al,
        span,
        ..
    } = prem_al;
    internalize_iter_kind(prem_kind_al, span, iter_prems)
}

fn internalize_iter_kind(
    prem_kind_al: al::PremKind,
    span: Span,
    iter_prems: Vec<al::PremIter>,
) -> (al::Prem, Vec<al::PremIter>) {
    match prem_kind_al {
        al::PremKind::Iter(prem_iter_al) => internalize_iter_prem(prem_iter_al, iter_prems),
        prem_kind_al => (crate::phrase! {node: prem_kind_al, span: span}, iter_prems),
    }
}

fn internalize_iter_prem(
    prem_iter_al: al::IterPrem,
    mut iter_prems: Vec<al::PremIter>,
) -> (al::Prem, Vec<al::PremIter>) {
    let al::IterPrem { prem, prem_iter } = prem_iter_al;
    iter_prems.insert(0, prem_iter);
    internalize_iter(*prem, iter_prems)
}

fn struct_prems(
    prems_al: Vec<al::Prem>,
    instr_ret: ol::Instr,
) -> Result<ol::Instr, StructureError> {
    let mut prems_internalized = prems_al
        .into_iter()
        .map(|prem_al| internalize_iter(prem_al, vec![]));
    struct_prems_inner(&mut prems_internalized, instr_ret)
}

fn struct_prems_inner(
    prems_internalized: &mut impl Iterator<Item = (al::Prem, Vec<al::PremIter>)>,
    instr_ret: ol::Instr,
) -> Result<ol::Instr, StructureError> {
    let Some((prem_al, iter_prems)) = prems_internalized.next() else {
        return Ok(instr_ret);
    };
    let Phrase {
        node: prem_kind_al,
        span,
        ..
    } = prem_al;
    let instr_kind_ol = struct_prem_kind(
        prem_kind_al,
        &span,
        iter_prems,
        prems_internalized,
        instr_ret,
    )?;
    Ok(crate::phrase! {node: instr_kind_ol, span: span})
}

fn struct_prem_kind(
    prem_kind_al: al::PremKind,
    span: &Span,
    iter_prems: Vec<al::PremIter>,
    prems_tail: &mut impl Iterator<Item = (al::Prem, Vec<al::PremIter>)>,
    instr_ret: ol::Instr,
) -> Result<ol::InstrKind, StructureError> {
    match prem_kind_al {
        al::PremKind::Rule(prem_al) => {
            struct_rule_prem(prem_al, span, iter_prems, prems_tail, instr_ret)
        }
        al::PremKind::If(prem_al) => {
            struct_if_prem(prem_al, span, iter_prems, prems_tail, instr_ret)
        }
        al::PremKind::IfHold(prem_al) => {
            struct_if_hold_prem(prem_al, span, iter_prems, prems_tail, instr_ret)
        }
        al::PremKind::IfNotHold(prem_al) => {
            struct_if_not_hold_prem(prem_al, span, iter_prems, prems_tail, instr_ret)
        }
        al::PremKind::Let(prem_al) => struct_let_prem(prem_al, iter_prems, prems_tail, instr_ret),
        al::PremKind::Debug(prem_al) => struct_debug_prem(prem_al, prems_tail, instr_ret),
        al::PremKind::Iter(_) => Err(StructureError::new(
            StructureErrorKind::UnsupportedPremise,
            span.clone(),
        )),
    }
}

fn struct_rule_prem(
    prem_al: al::RulePrem,
    span: &Span,
    iter_instrs: Vec<al::PremIter>,
    prems_tail: &mut impl Iterator<Item = (al::Prem, Vec<al::PremIter>)>,
    instr_ret: ol::Instr,
) -> Result<ol::InstrKind, StructureError> {
    let al::RulePrem {
        id,
        not_exp,
        input_hint,
    } = prem_al;
    input::validate(&input_hint, not_exp.arity())
        .map_err(|error| StructureError::new(StructureErrorKind::Input(error), span.clone()))?;
    let instr_tail = struct_prems_inner(prems_tail, instr_ret)?;
    Ok(ol::InstrKind::Rule(ol::RuleInstr {
        id,
        not_exp,
        input_hint,
        iter_instrs,
        block: vec![instr_tail],
    }))
}

fn iter_exps_without_bindings(
    iter_prems: Vec<al::PremIter>,
    error_kind: StructureErrorKind,
    span: &Span,
) -> Result<Vec<al::ExpIter>, StructureError> {
    iter_prems
        .into_iter()
        .map(|prem_iter| {
            let al::PremIter {
                iter,
                vars_bound,
                vars_bind,
            } = prem_iter;
            if !vars_bind.is_empty() {
                return Err(StructureError::new(error_kind.clone(), span.clone()));
            }
            Ok((iter, vars_bound))
        })
        .collect()
}

fn struct_if_prem(
    prem_al: al::IfPrem,
    span: &Span,
    iter_prems: Vec<al::PremIter>,
    prems_tail: &mut impl Iterator<Item = (al::Prem, Vec<al::PremIter>)>,
    instr_ret: ol::Instr,
) -> Result<ol::InstrKind, StructureError> {
    let al::IfPrem { exp } = prem_al;
    let iter_exps =
        iter_exps_without_bindings(iter_prems, StructureErrorKind::UnexpectedIfBindings, span)?;
    let instr_tail = struct_prems_inner(prems_tail, instr_ret)?;
    Ok(ol::InstrKind::If(ol::IfInstr {
        exp,
        iter_exps,
        block: vec![instr_tail],
    }))
}

fn struct_if_hold_prem(
    prem_al: al::IfHoldPrem,
    span: &Span,
    iter_prems: Vec<al::PremIter>,
    prems_tail: &mut impl Iterator<Item = (al::Prem, Vec<al::PremIter>)>,
    instr_ret: ol::Instr,
) -> Result<ol::InstrKind, StructureError> {
    let al::IfHoldPrem { id, not_exp } = prem_al;
    let iter_exps = iter_exps_without_bindings(
        iter_prems,
        StructureErrorKind::UnexpectedIfHoldBindings,
        span,
    )?;
    let instr_tail = struct_prems_inner(prems_tail, instr_ret)?;
    Ok(ol::InstrKind::Hold(ol::HoldInstr {
        id,
        not_exp,
        iter_exps,
        block_hold: vec![instr_tail],
        block_not_hold: vec![],
    }))
}

fn struct_if_not_hold_prem(
    prem_al: al::IfNotHoldPrem,
    span: &Span,
    iter_prems: Vec<al::PremIter>,
    prems_tail: &mut impl Iterator<Item = (al::Prem, Vec<al::PremIter>)>,
    instr_ret: ol::Instr,
) -> Result<ol::InstrKind, StructureError> {
    let al::IfNotHoldPrem { id, not_exp } = prem_al;
    let iter_exps = iter_exps_without_bindings(
        iter_prems,
        StructureErrorKind::UnexpectedIfNotHoldBindings,
        span,
    )?;
    let instr_tail = struct_prems_inner(prems_tail, instr_ret)?;
    Ok(ol::InstrKind::Hold(ol::HoldInstr {
        id,
        not_exp,
        iter_exps,
        block_hold: vec![],
        block_not_hold: vec![instr_tail],
    }))
}

fn struct_let_prem(
    prem_al: al::LetPrem,
    iter_instrs: Vec<al::PremIter>,
    prems_tail: &mut impl Iterator<Item = (al::Prem, Vec<al::PremIter>)>,
    instr_ret: ol::Instr,
) -> Result<ol::InstrKind, StructureError> {
    let al::LetPrem { exp_l, exp_r } = prem_al;
    let instr_tail = struct_prems_inner(prems_tail, instr_ret)?;
    Ok(ol::InstrKind::Let(ol::LetInstr {
        exp_l,
        exp_r,
        iter_instrs,
        block: vec![instr_tail],
    }))
}

fn struct_debug_prem(
    prem_al: al::DebugPrem,
    prems_tail: &mut impl Iterator<Item = (al::Prem, Vec<al::PremIter>)>,
    instr_ret: ol::Instr,
) -> Result<ol::InstrKind, StructureError> {
    let al::DebugPrem { exp } = prem_al;
    let instr_tail = struct_prems_inner(prems_tail, instr_ret)?;
    Ok(ol::InstrKind::Debug(ol::DebugInstr {
        exp,
        instr: Box::new(instr_tail),
    }))
}

// Rules

fn struct_rule_paths(
    rel_signature: &ol::RelSignature,
    rule_path: al::RulePath,
) -> Result<ol::Block, StructureError> {
    let al::RulePath {
        prems, exps_output, ..
    } = rule_path;
    let span = if exps_output.is_empty() {
        if prems.is_empty() {
            rel_signature.not_typ.span.clone()
        } else {
            Span::over(
                &prems
                    .iter()
                    .map(|prem| prem.span.clone())
                    .collect::<Vec<_>>(),
            )
        }
    } else {
        Span::over(
            &exps_output
                .iter()
                .map(|exp| exp.span.clone())
                .collect::<Vec<_>>(),
        )
    };
    let instr_result = crate::phrase! {node: ol::InstrKind::Result(ol::ResultInstr {rel_signature: rel_signature.clone(), exps: exps_output}), span: span};
    Ok(vec![struct_prems(prems, instr_result)?])
}

fn struct_rule_group(
    rel_signature: &ol::RelSignature,
    mut prems_unified: Vec<al::Prem>,
    rule_group: al::RuleGroup,
) -> Result<ol::Block, StructureError> {
    let Phrase {
        node: rule_group_kind,
        ..
    } = rule_group;
    let al::RuleGroupKind {
        id,
        rule_match,
        rule_paths,
    } = rule_group_kind;
    let al::RuleMatch {
        exps_signature,
        prems,
        ..
    } = rule_match;
    prems_unified.extend(prems);
    let blocks = rule_paths
        .into_iter()
        .map(|rule_path| struct_rule_paths(rel_signature, rule_path))
        .collect::<Result<_, _>>()?;
    let block = merge::merge_blocks(blocks);
    let span = id.span.clone();
    let instr_group = crate::phrase! {node: ol::InstrKind::Group(ol::GroupInstr {id, rel_signature: rel_signature.clone(), exps: exps_signature, block}), span: span};
    Ok(vec![struct_prems(prems_unified, instr_group)?])
}

fn struct_else_group(
    rel_signature: &ol::RelSignature,
    mut prems_unified: Vec<al::Prem>,
    else_group: al::ElseGroup,
) -> Result<ol::Block, StructureError> {
    let Phrase {
        node: else_group_kind,
        ..
    } = else_group;
    let al::ElseGroupKind {
        id,
        rule_match,
        rule_path,
    } = else_group_kind;
    let al::RuleMatch {
        exps_signature,
        prems,
        ..
    } = rule_match;
    prems_unified.extend(prems);
    let block = struct_rule_paths(rel_signature, rule_path)?;
    let span = id.span.clone();
    let instr_group = crate::phrase! {node: ol::InstrKind::Group(ol::GroupInstr {id, rel_signature: rel_signature.clone(), exps: exps_signature, block}), span: span};
    Ok(vec![struct_prems(prems_unified, instr_group)?])
}

// Clauses and table rows

fn struct_clause_path(path: ClausePath) -> Result<ol::Block, StructureError> {
    let ClausePath { prems, exp } = path;
    let span = exp.span.clone();
    let instr_return =
        crate::phrase! {node: ol::InstrKind::Return(ol::ReturnInstr {exp}), span: span};
    Ok(vec![struct_prems(prems, instr_return)?])
}

// Definitions

fn struct_def(
    ctx: &Context,
    def_al: al::Def,
    without_rule_groups: bool,
) -> Result<sl::Def, StructureError> {
    let Phrase {
        node: def_kind_al,
        span,
        ..
    } = def_al;
    let def_kind_sl = struct_def_kind(ctx, def_kind_al, &span, without_rule_groups)?;
    Ok(crate::phrase! {node: def_kind_sl, span: span})
}

fn struct_def_kind(
    ctx: &Context,
    def_kind_al: al::DefKind,
    span: &Span,
    without_rule_groups: bool,
) -> Result<sl::DefKind, StructureError> {
    match def_kind_al {
        al::DefKind::Typ(typdef_al) => Ok(sl::DefKind::Typ(struct_typ_def(typdef_al))),
        al::DefKind::Var(def_var_al) => Ok(sl::DefKind::Var(struct_var_def(def_var_al))),
        al::DefKind::Rel(def_rel_al) => {
            struct_rel_def(ctx, def_rel_al, span, without_rule_groups).map(sl::DefKind::Rel)
        }
        al::DefKind::MetaFunc(def_func_al) => {
            struct_func_def(ctx, def_func_al, span, without_rule_groups).map(sl::DefKind::MetaFunc)
        }
    }
}

fn struct_typ_def(typdef_al: al::TypDef) -> sl::TypDef {
    match typdef_al {
        al::TypDef::Extern(typdef_al) => sl::TypDef::Extern(struct_extern_typ_def(typdef_al)),
        al::TypDef::Defined(typdef_al) => {
            sl::TypDef::Defined(Box::new(struct_defined_typ_def(*typdef_al)))
        }
    }
}
fn struct_extern_typ_def(typdef_al: al::ExternTyp) -> sl::ExternTyp {
    let al::ExternTyp { id, hints } = typdef_al;
    sl::ExternTyp { id, hints }
}
fn struct_defined_typ_def(typdef_al: al::DefinedTyp) -> sl::DefinedTyp {
    let al::DefinedTyp {
        id,
        tparams,
        def_typ,
        hints,
    } = typdef_al;
    sl::DefinedTyp {
        id,
        tparams,
        def_typ,
        hints,
    }
}
fn struct_var_def(def_var_al: al::VarDef) -> sl::VarDef {
    let al::VarDef { id, typ, hints } = def_var_al;
    sl::VarDef { id, typ, hints }
}
fn struct_rel_def(
    ctx: &Context,
    def_rel_al: al::RelDef,
    span: &Span,
    without_rule_groups: bool,
) -> Result<sl::RelDef, StructureError> {
    match def_rel_al {
        al::RelDef::Extern(def_rel_al) => {
            struct_extern_rel_def(ctx, *def_rel_al, span).map(sl::RelDef::Extern)
        }
        al::RelDef::Defined(def_rel_al) => {
            struct_defined_rel_def(ctx, *def_rel_al, span, without_rule_groups)
                .map(sl::RelDef::Defined)
        }
    }
}
fn struct_func_def(
    ctx: &Context,
    def_func_al: al::MetaFuncDef,
    span: &Span,
    without_rule_groups: bool,
) -> Result<sl::MetaFuncDef, StructureError> {
    match def_func_al {
        al::MetaFuncDef::Extern(def_func_al) => Ok(sl::MetaFuncDef::Extern(struct_extern_dec_def(
            ctx,
            def_func_al,
        ))),
        al::MetaFuncDef::Builtin(def_func_al) => Ok(sl::MetaFuncDef::Builtin(
            struct_builtin_dec_def(ctx, def_func_al),
        )),
        al::MetaFuncDef::Table(def_func_al) => {
            struct_table_dec_def(ctx, def_func_al, span, without_rule_groups)
                .map(sl::MetaFuncDef::Table)
        }
        al::MetaFuncDef::Defined(def_func_al) => {
            struct_func_dec_def(ctx, *def_func_al, span, without_rule_groups)
                .map(sl::MetaFuncDef::Defined)
        }
    }
}

// Relation definitions

fn fresh_rel_inputs(
    ctx: &Context,
    not_typ: &al::NotTyp,
    input_hint: &input::InputHint,
    span: &Span,
) -> Result<Vec<al::Exp>, StructureError> {
    let typs = not_typ.node.args();
    input::validate(input_hint, typs.len())
        .map_err(|error| StructureError::new(StructureErrorKind::Input(error), span.clone()))?;
    let mut frees = IdSet::new();
    let mut exps_input = vec![];
    for int_idx in input_hint.indices() {
        let typ = typs[*int_idx as usize];
        let (frees_next, exp_input) = fresh::exp_from_typ(true, &ctx.menv, &frees, typ);
        frees = frees_next;
        exps_input.push(exp_input);
    }
    Ok(exps_input)
}
fn struct_extern_rel_def(
    ctx: &Context,
    def_rel_al: al::ExternRel,
    span: &Span,
) -> Result<sl::ExternRel, StructureError> {
    let al::ExternRel {
        id,
        not_typ,
        input_hint,
        hints,
    } = def_rel_al;
    let exps_input = fresh_rel_inputs(ctx, &not_typ, &input_hint, span)?;
    let rel_signature = sl::RelSignature {
        not_typ,
        input_hint,
    };
    Ok(sl::ExternRel {
        id,
        rel_signature,
        exps_input,
        hints,
    })
}
fn struct_defined_rel_def(
    ctx: &Context,
    def_rel_al: al::DefinedRel,
    span: &Span,
    without_rule_groups: bool,
) -> Result<sl::DefinedRel, StructureError> {
    let frees = def_rel_al.free();
    let al::DefinedRel {
        id,
        not_typ,
        input_hint,
        rule_groups,
        else_group,
        hints,
    } = def_rel_al;
    input::validate(&input_hint, not_typ.node.arity())
        .map_err(|error| StructureError::new(StructureErrorKind::Input(error), span.clone()))?;
    let exps_match_group = rule_groups
        .iter()
        .map(|rule_group| rule_group.node.rule_match.exps_input.clone())
        .collect::<Vec<_>>();
    let exps_match_else = else_group
        .as_ref()
        .map(|else_group| else_group.node.rule_match.exps_input.as_slice());
    let unified = if rule_groups.is_empty() && else_group.is_none() {
        antiunify::RuleMatchGroup {
            exps_template: fresh_rel_inputs(ctx, &not_typ, &input_hint, span)?,
            prems_group: vec![],
            prems_else: None,
        }
    } else {
        antiunify::antiunify_rule_match_group(frees, &exps_match_group, exps_match_else)?
    };
    let rel_signature = sl::RelSignature {
        not_typ,
        input_hint,
    };
    let blocks = unified
        .prems_group
        .into_iter()
        .zip(rule_groups)
        .map(|(prems, rule_group)| struct_rule_group(&rel_signature, prems, rule_group))
        .collect::<Result<_, _>>()?;
    let block = merge::merge_blocks(blocks);
    let block_else = match (unified.prems_else, else_group) {
        (Some(prems), Some(else_group)) => {
            Some(struct_else_group(&rel_signature, prems, else_group)?)
        }
        _ => None,
    };
    let blocks = optimize::optimize_with_else(&ctx.tdenv, block, block_else, without_rule_groups)?;
    let blocks = totalize::totalize(&ctx.tdenv, blocks.block, blocks.block_else)?;
    let body = prettify::pretty_rel(unified.exps_template, blocks.block, blocks.block_else)?;
    let blocks_sl = dangle::instrument(body.block, body.block_else)?;
    Ok(sl::DefinedRel {
        id,
        rel_signature,
        exps_input: body.exps_match,
        block: blocks_sl.block,
        block_else: blocks_sl.block_else,
        hints,
    })
}

// Function definitions

fn struct_extern_dec_def(ctx: &Context, def_func_al: al::ExternFunc) -> sl::ExternFunc {
    let al::ExternFunc {
        id,
        tparams,
        params: params_al,
        typ,
        hints,
    } = def_func_al;
    let params_sl = struct_params(ctx, params_al);
    sl::ExternFunc {
        id,
        tparams,
        params: params_sl,
        typ,
        hints,
    }
}
fn struct_builtin_dec_def(ctx: &Context, def_func_al: al::BuiltinFunc) -> sl::BuiltinFunc {
    let al::BuiltinFunc {
        id,
        tparams,
        params: params_al,
        typ,
        hints,
    } = def_func_al;
    let params_sl = struct_params(ctx, params_al);
    sl::BuiltinFunc {
        id,
        tparams,
        params: params_sl,
        typ,
        hints,
    }
}
fn struct_table_dec_def(
    ctx: &Context,
    def_func_al: al::TableFunc,
    span: &Span,
    without_rule_groups: bool,
) -> Result<sl::TableFunc, StructureError> {
    let al::TableFunc {
        id,
        params: params_al,
        typ,
        table_rows: table_rows_al,
        hints,
    } = def_func_al;
    let (exps_signature_group, clauses): (Vec<_>, Vec<_>) = table_rows_al
        .into_iter()
        .map(struct_table_row_clause)
        .unzip();
    let clauses = antiunify::antiunify_clauses(clauses, None)?;
    let params_sl = struct_params_from_args(ctx, params_al, clauses.args_template, span)?;
    let exps_output = clauses
        .paths
        .iter()
        .map(|path| path.exp.clone())
        .collect::<Vec<_>>();
    let blocks_ol = clauses
        .paths
        .into_iter()
        .map(struct_clause_path)
        .collect::<Result<Vec<_>, _>>()?;
    // Finish each phase across all rows before entering the next phase
    let blocks_ol = blocks_ol
        .into_iter()
        .map(|block_ol| optimize::optimize_without_else(&ctx.tdenv, block_ol, without_rule_groups))
        .collect::<Result<Vec<_>, _>>()?;
    let blocks_ol = blocks_ol
        .into_iter()
        .map(|block_ol| totalize::totalize_without_else(&ctx.tdenv, block_ol))
        .collect::<Result<Vec<_>, _>>()?;
    let blocks_sl = blocks_ol
        .into_iter()
        .map(dangle::instrument_without_else)
        .collect::<Result<Vec<_>, _>>()?;
    let table_rows_sl = exps_signature_group
        .into_iter()
        .zip(exps_output)
        .zip(blocks_sl)
        .map(|((exps_input, exp), block)| sl::TableRow {
            exps_input,
            exp,
            block,
        })
        .collect();
    Ok(sl::TableFunc {
        id,
        params: params_sl,
        typ,
        table_rows: table_rows_sl,
        hints,
    })
}
fn struct_table_row_clause(table_row_al: al::TableRow) -> (Vec<al::Exp>, al::Clause) {
    let Phrase {
        node: table_row_kind_al,
        span,
        ..
    } = table_row_al;
    let al::TableRowKind {
        exps_signature,
        args,
        exp,
        prems,
    } = table_row_kind_al;
    let clause = crate::phrase! {node: al::ClauseKind {args, exp, prems}, span: span};
    (exps_signature, clause)
}
fn struct_func_dec_def(
    ctx: &Context,
    def_func_al: al::DefinedFunc,
    span: &Span,
    without_rule_groups: bool,
) -> Result<sl::DefinedFunc, StructureError> {
    let al::DefinedFunc {
        id,
        tparams,
        params: params_al,
        typ,
        clauses,
        else_clause,
        hints,
    } = def_func_al;
    let clauses = antiunify::antiunify_clauses(clauses, else_clause)?;
    if clauses.paths.is_empty() && clauses.path_else.is_none() {
        let params_sl = struct_params(ctx, params_al);
        return Ok(sl::DefinedFunc {
            id,
            tparams,
            params: params_sl,
            typ,
            block: vec![],
            block_else: None,
            hints,
        });
    }
    let blocks = clauses
        .paths
        .into_iter()
        .map(struct_clause_path)
        .collect::<Result<_, _>>()?;
    let block = merge::merge_blocks(blocks);
    let block_else = clauses.path_else.map(struct_clause_path).transpose()?;
    let blocks = optimize::optimize_with_else(&ctx.tdenv, block, block_else, without_rule_groups)?;
    let blocks = totalize::totalize(&ctx.tdenv, blocks.block, blocks.block_else)?;
    let body = prettify::pretty_func(clauses.args_template, blocks.block, blocks.block_else)?;
    let params_sl = struct_params_from_args(ctx, params_al, body.args_input, span)?;
    let blocks_sl = dangle::instrument(body.block, body.block_else)?;
    Ok(sl::DefinedFunc {
        id,
        tparams,
        params: params_sl,
        typ,
        block: blocks_sl.block,
        block_else: blocks_sl.block_else,
        hints,
    })
}

fn check_arity(num_expect: usize, num_actual: usize, span: &Span) -> Result<(), StructureError> {
    if num_expect != num_actual {
        return Err(StructureError::new(
            StructureErrorKind::ArityMismatch {
                expected: num_expect,
                actual: num_actual,
            },
            span.clone(),
        ));
    }
    Ok(())
}

/// Converts algorithmic definitions, removing rule groups when requested
pub fn convert(spec_al: al::Spec, without_rule_groups: bool) -> Result<sl::Spec, StructureError> {
    let ctx = Context::load(&spec_al)?;
    spec_al
        .into_iter()
        .map(|def_al| struct_def(&ctx, def_al, without_rule_groups))
        .collect()
}
