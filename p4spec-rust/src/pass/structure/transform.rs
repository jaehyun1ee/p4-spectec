//! Convert AL premises to nested OL blocks, then optimize and lower them to SL
//!
//! `let y = x; if y > 0; return y` becomes `Let(y, x, [If(y > 0,
//! [Return(y)])])` in OL and `If(x > 0, [Return(x)], dangle=true)` in SL
//!
//! AL -> OL -> optimize -> totalize -> prettify -> SL with fallthrough flags

use super::{
    StructureError, StructureErrorKind, antiunify, context::Context, dangle, merge, ol::ast as ol,
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

// == Parameters

// - Parameter

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

fn struct_params(ctx: &Context, params_al: Vec<al::Param>) -> Vec<sl::Param> {
    let mut frees = IdSet::new();
    params_al
        .into_iter()
        .map(|param_al| struct_param(ctx, &mut frees, param_al))
        .collect()
}

// - Expression parameter

fn struct_exp_param(ctx: &Context, frees: &mut IdSet, typ: al::Typ) -> sl::ParamKind {
    let (frees_next, exp_input) = fresh::exp_from_typ(true, &ctx.menv, frees, &typ);
    *frees = frees_next;
    let exp_input = Box::new(exp_input);
    sl::ParamKind::Exp(typ, exp_input)
}

// - Definition parameter

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

// == Parameters from arguments

// - Parameter from argument

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
    let param_sl = crate::phrase! {node: param_kind_sl, span: span};
    Ok(param_sl)
}

fn struct_param_kind_from_arg(
    ctx: &Context,
    param_kind_al: al::ParamKind,
    arg_kind: al::ArgKind,
    span: &Span,
) -> Result<sl::ParamKind, StructureError> {
    match (param_kind_al, arg_kind) {
        (al::ParamKind::Exp(typ), al::ArgKind::Exp(exp)) => {
            let param_kind_sl = sl::ParamKind::Exp(typ, exp);
            Ok(param_kind_sl)
        }
        (al::ParamKind::Def(id, tparams, params_al, typ), al::ArgKind::Def(id_arg)) => {
            struct_def_param_from_arg(ctx, id, tparams, params_al, typ, id_arg, span)
        }
        _ => {
            let error_kind = StructureErrorKind::IncompatibleParameterArgument;
            let error = StructureError::new(error_kind, span.clone());
            Err(error)
        }
    }
}

fn struct_params_from_args(
    ctx: &Context,
    params_al: Vec<al::Param>,
    args_input: Vec<al::Arg>,
    span: &Span,
) -> Result<Vec<sl::Param>, StructureError> {
    if params_al.len() != args_input.len() {
        let error_kind = StructureErrorKind::ArityMismatch {
            expected: params_al.len(),
            actual: args_input.len(),
        };
        let error = StructureError::new(error_kind, span.clone());
        return Err(error);
    }
    params_al
        .into_iter()
        .zip(args_input)
        .map(|(param_al, arg_input)| struct_param_from_arg(ctx, param_al, arg_input))
        .collect()
}

// - Definition parameter from argument

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
        let error_kind = StructureErrorKind::IncompatibleParameterArgument;
        let error = StructureError::new(error_kind, span.clone());
        return Err(error);
    }
    let param_kind_sl = struct_def_param(ctx, id, tparams, params_al, typ);
    Ok(param_kind_sl)
}

// == Premises

// - Premise

fn struct_prem(
    prem_al: al::Prem,
    prems_tail: &mut impl Iterator<Item = al::Prem>,
    instr_ret: ol::Instr,
) -> Result<ol::Instr, StructureError> {
    let (prem_al, iter_prems) = internalize_iter(prem_al);
    let Phrase {
        node: prem_kind_al,
        span,
        ..
    } = prem_al;
    let instr_kind_ol = struct_prem_kind(prem_kind_al, &span, iter_prems, prems_tail, instr_ret)?;
    let instr_ol = crate::phrase! {node: instr_kind_ol, span: span};
    Ok(instr_ol)
}

fn internalize_iter(mut prem_al: al::Prem) -> (al::Prem, Vec<al::PremIter>) {
    let mut iter_prems = vec![];
    loop {
        let Phrase {
            node: prem_kind_al,
            span,
            ..
        } = prem_al;
        match prem_kind_al {
            al::PremKind::Iter(prem_iter_al) => {
                let al::IterPrem { prem, prem_iter } = prem_iter_al;
                iter_prems.push(prem_iter);
                prem_al = *prem;
            }
            prem_kind_al => {
                // The innermost iterator becomes the first instruction iterator
                iter_prems.reverse();
                let prem_al = crate::phrase!(node: prem_kind_al, span: span);
                return (prem_al, iter_prems);
            }
        }
    }
}

fn struct_prem_kind(
    prem_kind_al: al::PremKind,
    span: &Span,
    iter_prems: Vec<al::PremIter>,
    prems_tail: &mut impl Iterator<Item = al::Prem>,
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
        al::PremKind::Iter(_) => unreachable!("iterators were internalized before structuring"),
    }
}

fn struct_prems(
    prems_al: &mut impl Iterator<Item = al::Prem>,
    instr_ret: ol::Instr,
) -> Result<ol::Instr, StructureError> {
    match prems_al.next() {
        Some(prem_al) => struct_prem(prem_al, prems_al, instr_ret),
        None => Ok(instr_ret),
    }
}

// - Demoting premise iterators (with bindings) to expression iterators (without bindings)

fn demote_iter_prems(
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
                let error = StructureError::new(error_kind.clone(), span.clone());
                return Err(error);
            }
            Ok((iter, vars_bound))
        })
        .collect()
}

// - Rule premise

fn struct_rule_prem(
    prem_al: al::RulePrem,
    span: &Span,
    iter_instrs: Vec<al::PremIter>,
    prems_tail: &mut impl Iterator<Item = al::Prem>,
    instr_ret: ol::Instr,
) -> Result<ol::InstrKind, StructureError> {
    let al::RulePrem {
        id,
        not_exp,
        input_hint,
    } = prem_al;
    input::validate(&input_hint, not_exp.arity()).map_err(|error| {
        let error_kind = StructureErrorKind::Input(error);
        StructureError::new(error_kind, span.clone())
    })?;
    let instr_tail = struct_prems(prems_tail, instr_ret)?;
    let block = vec![instr_tail];
    let instr_ol = ol::RuleInstr {
        id,
        not_exp,
        input_hint,
        iter_instrs,
        block,
    };
    let instr_kind_ol = ol::InstrKind::Rule(instr_ol);
    Ok(instr_kind_ol)
}

// - If premise

fn struct_if_prem(
    prem_al: al::IfPrem,
    span: &Span,
    iter_prems: Vec<al::PremIter>,
    prems_tail: &mut impl Iterator<Item = al::Prem>,
    instr_ret: ol::Instr,
) -> Result<ol::InstrKind, StructureError> {
    let al::IfPrem { exp } = prem_al;
    let iter_exps = demote_iter_prems(iter_prems, StructureErrorKind::UnexpectedIfBindings, span)?;
    let instr_tail = struct_prems(prems_tail, instr_ret)?;
    let block = vec![instr_tail];
    let instr_ol = ol::IfInstr {
        exp,
        iter_exps,
        block,
    };
    let instr_kind_ol = ol::InstrKind::If(instr_ol);
    Ok(instr_kind_ol)
}

// - If-hold premise

fn struct_if_hold_prem(
    prem_al: al::IfHoldPrem,
    span: &Span,
    iter_prems: Vec<al::PremIter>,
    prems_tail: &mut impl Iterator<Item = al::Prem>,
    instr_ret: ol::Instr,
) -> Result<ol::InstrKind, StructureError> {
    let al::IfHoldPrem { id, not_exp } = prem_al;
    let iter_exps = demote_iter_prems(
        iter_prems,
        StructureErrorKind::UnexpectedIfHoldBindings,
        span,
    )?;
    let instr_tail = struct_prems(prems_tail, instr_ret)?;
    let block_hold = vec![instr_tail];
    let instr_ol = ol::HoldInstr {
        id,
        not_exp,
        iter_exps,
        block_hold,
        block_not_hold: vec![],
    };
    let instr_kind_ol = ol::InstrKind::Hold(instr_ol);
    Ok(instr_kind_ol)
}

// - If-not-hold premise

fn struct_if_not_hold_prem(
    prem_al: al::IfNotHoldPrem,
    span: &Span,
    iter_prems: Vec<al::PremIter>,
    prems_tail: &mut impl Iterator<Item = al::Prem>,
    instr_ret: ol::Instr,
) -> Result<ol::InstrKind, StructureError> {
    let al::IfNotHoldPrem { id, not_exp } = prem_al;
    let iter_exps = demote_iter_prems(
        iter_prems,
        StructureErrorKind::UnexpectedIfNotHoldBindings,
        span,
    )?;
    let instr_tail = struct_prems(prems_tail, instr_ret)?;
    let block_not_hold = vec![instr_tail];
    let instr_ol = ol::HoldInstr {
        id,
        not_exp,
        iter_exps,
        block_hold: vec![],
        block_not_hold,
    };
    let instr_kind_ol = ol::InstrKind::Hold(instr_ol);
    Ok(instr_kind_ol)
}

// - Let premise

fn struct_let_prem(
    prem_al: al::LetPrem,
    iter_instrs: Vec<al::PremIter>,
    prems_tail: &mut impl Iterator<Item = al::Prem>,
    instr_ret: ol::Instr,
) -> Result<ol::InstrKind, StructureError> {
    let al::LetPrem { exp_l, exp_r } = prem_al;
    let instr_tail = struct_prems(prems_tail, instr_ret)?;
    let block = vec![instr_tail];
    let instr_ol = ol::LetInstr {
        exp_l,
        exp_r,
        iter_instrs,
        block,
    };
    let instr_kind_ol = ol::InstrKind::Let(instr_ol);
    Ok(instr_kind_ol)
}

// - Debug premise

fn struct_debug_prem(
    prem_al: al::DebugPrem,
    prems_tail: &mut impl Iterator<Item = al::Prem>,
    instr_ret: ol::Instr,
) -> Result<ol::InstrKind, StructureError> {
    let al::DebugPrem { exp } = prem_al;
    let instr_tail = struct_prems(prems_tail, instr_ret)?;
    let instr_tail = Box::new(instr_tail);
    let instr_ol = ol::DebugInstr {
        exp,
        instr: instr_tail,
    };
    let instr_kind_ol = ol::InstrKind::Debug(instr_ol);
    Ok(instr_kind_ol)
}

// == Rules

// - Rule path

fn struct_rule_path(
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
            let spans = prems
                .iter()
                .map(|prem| prem.span.clone())
                .collect::<Vec<_>>();
            Span::over(&spans)
        }
    } else {
        let spans = exps_output
            .iter()
            .map(|exp| exp.span.clone())
            .collect::<Vec<_>>();
        Span::over(&spans)
    };
    let instr_ol = ol::ResultInstr {
        rel_signature: rel_signature.clone(),
        exps: exps_output,
    };
    let instr_kind_ol = ol::InstrKind::Result(instr_ol);
    let instr_result = crate::phrase! {node: instr_kind_ol, span: span};
    let mut prems_al = prems.into_iter();
    let instr_ol = struct_prems(&mut prems_al, instr_result)?;
    let block = vec![instr_ol];
    Ok(block)
}

// - Rule group

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
        .map(|rule_path| struct_rule_path(rel_signature, rule_path))
        .collect::<Result<_, _>>()?;
    let block = merge::merge_blocks(blocks);
    let span = id.span.clone();
    let instr_ol = ol::GroupInstr {
        id,
        rel_signature: rel_signature.clone(),
        exps: exps_signature,
        block,
    };
    let instr_kind_ol = ol::InstrKind::Group(instr_ol);
    let instr_group = crate::phrase! {node: instr_kind_ol, span: span};
    let mut prems_al = prems_unified.into_iter();
    let instr_ol = struct_prems(&mut prems_al, instr_group)?;
    let block = vec![instr_ol];
    Ok(block)
}

// - Else group

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
    let block = struct_rule_path(rel_signature, rule_path)?;
    let span = id.span.clone();
    let instr_ol = ol::GroupInstr {
        id,
        rel_signature: rel_signature.clone(),
        exps: exps_signature,
        block,
    };
    let instr_kind_ol = ol::InstrKind::Group(instr_ol);
    let instr_group = crate::phrase! {node: instr_kind_ol, span: span};
    let mut prems_al = prems_unified.into_iter();
    let instr_ol = struct_prems(&mut prems_al, instr_group)?;
    let block = vec![instr_ol];
    Ok(block)
}

// == Clauses

// - Clause path

fn struct_clause_path((prems, exp): (Vec<al::Prem>, al::Exp)) -> Result<ol::Block, StructureError> {
    let span = exp.span.clone();
    let instr_ol = ol::ReturnInstr { exp };
    let instr_kind_ol = ol::InstrKind::Return(instr_ol);
    let instr_return = crate::phrase! {node: instr_kind_ol, span: span};
    let mut prems_al = prems.into_iter();
    let instr_ol = struct_prems(&mut prems_al, instr_return)?;
    let block = vec![instr_ol];
    Ok(block)
}

// == Table rows

// - Table row clause

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
    let clause_kind = al::ClauseKind { args, exp, prems };
    let clause = crate::phrase! {node: clause_kind, span: span};
    (exps_signature, clause)
}

// == Type definitions

// - Type definition

fn struct_typ_def(typdef_al: al::TypDef) -> sl::TypDef {
    match typdef_al {
        al::TypDef::Extern(typdef_al) => {
            let typdef_sl = struct_extern_typ_def(typdef_al);
            sl::TypDef::Extern(typdef_sl)
        }
        al::TypDef::Defined(typdef_al) => {
            let typdef_sl = struct_defined_typ_def(*typdef_al);
            let typdef_sl = Box::new(typdef_sl);
            sl::TypDef::Defined(typdef_sl)
        }
    }
}

// - External type definition

fn struct_extern_typ_def(typdef_al: al::ExternTyp) -> sl::ExternTyp {
    let al::ExternTyp { id, hints } = typdef_al;
    sl::ExternTyp { id, hints }
}

// - Defined type definition

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

// == Meta-variable definitions

// - Meta-variable definition

fn struct_var_def(def_var_al: al::VarDef) -> sl::VarDef {
    let al::VarDef { id, typ, hints } = def_var_al;
    sl::VarDef { id, typ, hints }
}

// == Relation definitions

// - Relation definition

fn struct_rel_def(
    ctx: &Context,
    def_rel_al: al::RelDef,
    span: &Span,
    without_rule_groups: bool,
) -> Result<sl::RelDef, StructureError> {
    let def_rel_sl = match def_rel_al {
        al::RelDef::Extern(def_rel_al) => {
            let def_rel_sl = struct_extern_rel_def(ctx, *def_rel_al, span)?;
            sl::RelDef::Extern(def_rel_sl)
        }
        al::RelDef::Defined(def_rel_al) => {
            let def_rel_sl = struct_defined_rel_def(ctx, *def_rel_al, span, without_rule_groups)?;
            sl::RelDef::Defined(def_rel_sl)
        }
    };
    Ok(def_rel_sl)
}

// - Fresh relation inputs

fn struct_rel_exps_input(
    ctx: &Context,
    not_typ: &al::NotTyp,
    input_hint: &input::InputHint,
    span: &Span,
) -> Result<Vec<al::Exp>, StructureError> {
    let typs = not_typ.node.args();
    input::validate(input_hint, typs.len()).map_err(|error| {
        let error_kind = StructureErrorKind::Input(error);
        StructureError::new(error_kind, span.clone())
    })?;
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

// - External relation definition

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
    let exps_input = struct_rel_exps_input(ctx, &not_typ, &input_hint, span)?;
    let rel_signature = sl::RelSignature {
        not_typ,
        input_hint,
    };
    let def_rel_sl = sl::ExternRel {
        id,
        rel_signature,
        exps_input,
        hints,
    };
    Ok(def_rel_sl)
}

// - Defined relation definition

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
    input::validate(&input_hint, not_typ.node.arity()).map_err(|error| {
        let error_kind = StructureErrorKind::Input(error);
        StructureError::new(error_kind, span.clone())
    })?;
    let exps_match_group = rule_groups
        .iter()
        .map(|rule_group| rule_group.node.rule_match.exps_input.clone())
        .collect::<Vec<_>>();
    let exps_match_else = else_group
        .as_ref()
        .map(|else_group| else_group.node.rule_match.exps_input.as_slice());
    let (exps_template, prems_group, prems_else) = if rule_groups.is_empty() && else_group.is_none()
    {
        let exps_input = struct_rel_exps_input(ctx, &not_typ, &input_hint, span)?;
        (exps_input, vec![], None)
    } else {
        antiunify::antiunify_rule_match_group(frees, &exps_match_group, exps_match_else)?
    };
    let rel_signature = sl::RelSignature {
        not_typ,
        input_hint,
    };
    let blocks = prems_group
        .into_iter()
        .zip(rule_groups)
        .map(|(prems, rule_group)| struct_rule_group(&rel_signature, prems, rule_group))
        .collect::<Result<_, _>>()?;
    let block = merge::merge_blocks(blocks);
    let block_else = match (prems_else, else_group) {
        (Some(prems), Some(else_group)) => {
            let block_else = struct_else_group(&rel_signature, prems, else_group)?;
            Some(block_else)
        }
        _ => None,
    };
    let (block, block_else) =
        optimize::optimize_with_else(&ctx.tdenv, block, block_else, without_rule_groups)?;
    let (block, block_else) = totalize::totalize(&ctx.tdenv, block, block_else)?;
    let (exps_input, block, block_else) = prettify::pretty_rel(exps_template, block, block_else)?;
    let (block, block_else) = dangle::instrument(block, block_else)?;
    let def_rel_sl = sl::DefinedRel {
        id,
        rel_signature,
        exps_input,
        block,
        block_else,
        hints,
    };
    Ok(def_rel_sl)
}

// == Meta-function definitions

// - Meta-function definition

fn struct_func_def(
    ctx: &Context,
    def_func_al: al::MetaFuncDef,
    span: &Span,
    without_rule_groups: bool,
) -> Result<sl::MetaFuncDef, StructureError> {
    let def_func_sl = match def_func_al {
        al::MetaFuncDef::Extern(def_func_al) => {
            let def_func_sl = struct_extern_dec_def(ctx, def_func_al);
            sl::MetaFuncDef::Extern(def_func_sl)
        }
        al::MetaFuncDef::Builtin(def_func_al) => {
            let def_func_sl = struct_builtin_dec_def(ctx, def_func_al);
            sl::MetaFuncDef::Builtin(def_func_sl)
        }
        al::MetaFuncDef::Table(def_func_al) => {
            let def_func_sl = struct_table_dec_def(ctx, def_func_al, span, without_rule_groups)?;
            sl::MetaFuncDef::Table(def_func_sl)
        }
        al::MetaFuncDef::Defined(def_func_al) => {
            let def_func_sl = struct_func_dec_def(ctx, *def_func_al, span, without_rule_groups)?;
            sl::MetaFuncDef::Defined(def_func_sl)
        }
    };
    Ok(def_func_sl)
}

// - External function declaration

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

// - Builtin function declaration

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

// - Table function declaration

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
    let (args_template, paths, _) = antiunify::antiunify_clauses(clauses, None)?;
    let params_sl = struct_params_from_args(ctx, params_al, args_template, span)?;
    let exps_output = paths.iter().map(|(_, exp)| exp.clone()).collect::<Vec<_>>();
    let blocks_ol = paths
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
    let def_func_sl = sl::TableFunc {
        id,
        params: params_sl,
        typ,
        table_rows: table_rows_sl,
        hints,
    };
    Ok(def_func_sl)
}

// - Function declaration

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
    let (args_template, paths, path_else) = antiunify::antiunify_clauses(clauses, else_clause)?;
    if paths.is_empty() && path_else.is_none() {
        let params_sl = struct_params(ctx, params_al);
        let def_func_sl = sl::DefinedFunc {
            id,
            tparams,
            params: params_sl,
            typ,
            block: vec![],
            block_else: None,
            hints,
        };
        return Ok(def_func_sl);
    }
    let blocks = paths
        .into_iter()
        .map(struct_clause_path)
        .collect::<Result<_, _>>()?;
    let block = merge::merge_blocks(blocks);
    let block_else = path_else.map(struct_clause_path).transpose()?;
    let (block, block_else) =
        optimize::optimize_with_else(&ctx.tdenv, block, block_else, without_rule_groups)?;
    let (block, block_else) = totalize::totalize(&ctx.tdenv, block, block_else)?;
    let (args_input, block, block_else) = prettify::pretty_func(args_template, block, block_else)?;
    let params_sl = struct_params_from_args(ctx, params_al, args_input, span)?;
    let (block, block_else) = dangle::instrument(block, block_else)?;
    let def_func_sl = sl::DefinedFunc {
        id,
        tparams,
        params: params_sl,
        typ,
        block,
        block_else,
        hints,
    };
    Ok(def_func_sl)
}

// == Definitions

// - Definition

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
    let def_sl = crate::phrase! {node: def_kind_sl, span: span};
    Ok(def_sl)
}

fn struct_def_kind(
    ctx: &Context,
    def_kind_al: al::DefKind,
    span: &Span,
    without_rule_groups: bool,
) -> Result<sl::DefKind, StructureError> {
    let def_kind_sl = match def_kind_al {
        al::DefKind::Typ(typdef_al) => {
            let typdef_sl = struct_typ_def(typdef_al);
            sl::DefKind::Typ(typdef_sl)
        }
        al::DefKind::Var(def_var_al) => {
            let def_var_sl = struct_var_def(def_var_al);
            sl::DefKind::Var(def_var_sl)
        }
        al::DefKind::Rel(def_rel_al) => {
            let def_rel_sl = struct_rel_def(ctx, def_rel_al, span, without_rule_groups)?;
            sl::DefKind::Rel(def_rel_sl)
        }
        al::DefKind::MetaFunc(def_func_al) => {
            let def_func_sl = struct_func_def(ctx, def_func_al, span, without_rule_groups)?;
            sl::DefKind::MetaFunc(def_func_sl)
        }
    };
    Ok(def_kind_sl)
}

// == Specification

// - Entry point

/// Converts algorithmic definitions, removing rule groups when requested
pub fn convert(spec_al: al::Spec, without_rule_groups: bool) -> Result<sl::Spec, StructureError> {
    let ctx = Context::load(&spec_al)?;
    spec_al
        .into_iter()
        .map(|def_al| struct_def(&ctx, def_al, without_rule_groups))
        .collect()
}
