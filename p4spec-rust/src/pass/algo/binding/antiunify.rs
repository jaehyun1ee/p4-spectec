//! Anti-unification of rule input expressions
//!
//! Overlap corresponding inputs to obtain a shared template, then add equality
//! premises for each rule. For example, `(true, x)` and `(false, x)` become
//! `(b, x)`, with `if b == true` and `if b == false` on the respective paths
//!
//! A failed structural overlap falls back to one fresh variable if the types
//! are equivalent. Fresh names from the failed attempt are discarded

use crate::{
    lang::{
        common::{ds::set::IdSet, notation::mixop::Mixop, source::Span},
        il::{ast, fresh, var},
        traits::eq::SyntaxEq,
        xl,
    },
    note_phrase, phrase,
    runtime::{
        envs::algo::{MEnv, TDEnv},
        ops::typ::equiv_typ,
    },
};

use super::{
    super::{AlgoError, AlgoErrorKind},
    context::Context,
};

// == Template overlap

// - Expressions

fn overlap_exp(
    tdenv: &TDEnv,
    menv: &MEnv,
    ids_free: &mut IdSet,
    ids_unifier: &mut IdSet,
    exp_template: &ast::Exp,
    exp: &ast::Exp,
) -> Result<ast::Exp, AlgoError> {
    if exp_template.syntax_eq(exp) {
        return Ok(exp_template.clone());
    }

    // Keep fresh names only when the entire structural overlap succeeds
    let mut ids_free_structural = ids_free.clone();
    let mut ids_unifier_structural = ids_unifier.clone();
    let exp_kind_template = overlap_exp_kind(
        tdenv,
        menv,
        &mut ids_free_structural,
        &mut ids_unifier_structural,
        exp_template,
        exp,
    );
    match exp_kind_template {
        Ok(exp_kind_template) => {
            *ids_free = ids_free_structural;
            *ids_unifier = ids_unifier_structural;
            let exp_template = note_phrase! {
                node: exp_kind_template,
                note: exp_template.note.clone(),
                span: exp_template.span.clone(),
            };
            return Ok(exp_template);
        }
        Err(error)
            if matches!(
                error.kind,
                AlgoErrorKind::AntiUnification | AlgoErrorKind::ExpressionArityMismatch { .. }
            ) => {}
        Err(error) => return Err(error),
    }

    let typ_template =
        phrase!(node: exp_template.note.as_ref().clone(), span: exp_template.span.clone());
    let typ = phrase!(node: exp.note.as_ref().clone(), span: exp.span.clone());
    let is_equivalent = equiv_typ(tdenv, &typ_template, &typ)?;
    if !is_equivalent {
        let error = AlgoError::new(AlgoErrorKind::AntiUnification, exp.span.clone());
        return Err(error);
    }
    let var_fresh = fresh::var_from_typ(menv, ids_free, exp_template.span.clone(), &typ_template);
    ids_free.insert(var_fresh.id.clone());
    ids_unifier.insert(var_fresh.id.clone());
    let exp_template = var::as_exp(true, &var_fresh);
    Ok(exp_template)
}

fn overlap_exp_kind(
    tdenv: &TDEnv,
    menv: &MEnv,
    ids_free: &mut IdSet,
    ids_unifier: &mut IdSet,
    exp_template: &ast::Exp,
    exp: &ast::Exp,
) -> Result<ast::ExpKind, AlgoError> {
    match (&exp_template.node, &exp.node) {
        (ast::ExpKind::Var(id_template), _) if ids_unifier.contains(id_template) => {
            Ok(exp_template.node.clone())
        }
        (
            ast::ExpKind::UpCast(typ_template, exp_template_inner),
            ast::ExpKind::UpCast(typ, exp_inner),
        ) if typ_template.syntax_eq(typ) => {
            let exp_template_inner = overlap_exp(
                tdenv,
                menv,
                ids_free,
                ids_unifier,
                exp_template_inner,
                exp_inner,
            )?;
            let exp_template_inner = Box::new(exp_template_inner);
            Ok(ast::ExpKind::UpCast(
                typ_template.clone(),
                exp_template_inner,
            ))
        }
        (ast::ExpKind::Tuple(exps_template), ast::ExpKind::Tuple(exps)) => {
            let exps_template = overlap_exps(
                tdenv,
                menv,
                ids_free,
                ids_unifier,
                exps_template.iter(),
                exps.iter(),
            )?;
            Ok(ast::ExpKind::Tuple(exps_template))
        }
        (ast::ExpKind::Case(not_exp_template), ast::ExpKind::Case(not_exp))
            if not_exp_template.eq_shape(not_exp) =>
        {
            overlap_case_exp(
                tdenv,
                menv,
                ids_free,
                ids_unifier,
                not_exp_template,
                not_exp,
            )
        }
        (ast::ExpKind::Str(expfields_template), ast::ExpKind::Str(expfields))
            if expfields_template.len() == expfields.len()
                && expfields_template
                    .iter()
                    .zip(expfields)
                    .all(|((atom_template, _), (atom, _))| atom_template.syntax_eq(atom)) =>
        {
            overlap_str_exp(
                tdenv,
                menv,
                ids_free,
                ids_unifier,
                expfields_template,
                expfields,
            )
        }
        _ => {
            let error = AlgoError::new(AlgoErrorKind::AntiUnification, exp.span.clone());
            Err(error)
        }
    }
}

fn overlap_exps<'a>(
    tdenv: &TDEnv,
    menv: &MEnv,
    ids_free: &mut IdSet,
    ids_unifier: &mut IdSet,
    exps_template: impl ExactSizeIterator<Item = &'a ast::Exp>,
    exps: impl ExactSizeIterator<Item = &'a ast::Exp>,
) -> Result<Vec<ast::Exp>, AlgoError> {
    if exps_template.len() != exps.len() {
        let kind = AlgoErrorKind::ExpressionArityMismatch {
            expected: exps_template.len(),
            actual: exps.len(),
        };
        let error = AlgoError::new(kind, Span::default());
        return Err(error);
    }
    let mut exps_overlapped = Vec::with_capacity(exps_template.len());
    for (exp_template, exp) in exps_template.zip(exps) {
        let exp_overlapped = overlap_exp(tdenv, menv, ids_free, ids_unifier, exp_template, exp)?;
        exps_overlapped.push(exp_overlapped);
    }
    Ok(exps_overlapped)
}

// - Case expression

fn overlap_case_exp(
    tdenv: &TDEnv,
    menv: &MEnv,
    ids_free: &mut IdSet,
    ids_unifier: &mut IdSet,
    not_exp_template: &ast::NotExp,
    not_exp: &ast::NotExp,
) -> Result<ast::ExpKind, AlgoError> {
    let (mixop, exps_template) = not_exp_template.split();
    let exps = not_exp.args();
    let exps_template = overlap_exps(
        tdenv,
        menv,
        ids_free,
        ids_unifier,
        exps_template.into_iter(),
        exps.into_iter(),
    )?;
    let not_exp_template = Mixop::fill(&mixop, exps_template)
        .expect("overlapped arguments must preserve the template mixfix arity");
    let not_exp_template = Box::new(not_exp_template);
    Ok(ast::ExpKind::Case(not_exp_template))
}

// - Record expression

fn overlap_str_exp(
    tdenv: &TDEnv,
    menv: &MEnv,
    ids_free: &mut IdSet,
    ids_unifier: &mut IdSet,
    expfields_template: &[ast::ExpField],
    expfields: &[ast::ExpField],
) -> Result<ast::ExpKind, AlgoError> {
    let exps_template = expfields_template.iter().map(|(_, exp)| exp);
    let exps = expfields.iter().map(|(_, exp)| exp);
    let exps_template = overlap_exps(tdenv, menv, ids_free, ids_unifier, exps_template, exps)?;
    let expfields_template = expfields_template
        .iter()
        .map(|(atom, _)| atom.clone())
        .zip(exps_template)
        .collect();
    Ok(ast::ExpKind::Str(expfields_template))
}

// - Expressions across rules

fn overlap_exp_across_rules<'a>(
    tdenv: &TDEnv,
    menv: &MEnv,
    ids_free: &mut IdSet,
    exp_template: &ast::Exp,
    exps: impl Iterator<Item = &'a ast::Exp>,
) -> Result<(IdSet, ast::Exp), AlgoError> {
    let mut ids_unifier = IdSet::new();
    let mut exp_template = exp_template.clone();
    for exp in exps {
        exp_template = overlap_exp(tdenv, menv, ids_free, &mut ids_unifier, &exp_template, exp)?;
    }
    Ok((ids_unifier, exp_template))
}

fn overlap_exps_across_rules(
    tdenv: &TDEnv,
    menv: &MEnv,
    ids_free: &mut IdSet,
    exps_by_rule: &[Vec<ast::Exp>],
) -> Result<(IdSet, Vec<ast::Exp>), AlgoError> {
    let Some((exps_head, exps_tail)) = exps_by_rule.split_first() else {
        return Ok((IdSet::new(), vec![]));
    };
    for exps in exps_tail {
        if exps.len() != exps_head.len() {
            let kind = AlgoErrorKind::ExpressionArityMismatch {
                expected: exps_head.len(),
                actual: exps.len(),
            };
            let error = AlgoError::new(kind, Span::default());
            return Err(error);
        }
    }
    if exps_tail.is_empty() {
        return Ok((IdSet::new(), exps_head.clone()));
    }

    let mut ids_unifier = IdSet::new();
    let mut exps_template = Vec::with_capacity(exps_head.len());
    for (idx, exp_head) in exps_head.iter().enumerate() {
        let exps_at_idx = exps_tail.iter().map(|exps| &exps[idx]);
        let (ids_unifier_exp, exp_template) =
            overlap_exp_across_rules(tdenv, menv, ids_free, exp_head, exps_at_idx)?;
        ids_unifier.append(ids_unifier_exp);
        exps_template.push(exp_template);
    }
    Ok((ids_unifier, exps_template))
}

// == Template population

// - Expressions

fn populate_exp(ids_unifier: &IdSet, exp_template: &ast::Exp, exp: &ast::Exp) -> Vec<ast::Prem> {
    if exp_template.syntax_eq(exp) {
        return vec![];
    }
    match (&exp_template.node, &exp.node) {
        (ast::ExpKind::Var(id_template), _) if ids_unifier.contains(id_template) => {
            let prem = populate_equality_prem(exp_template, exp);
            vec![prem]
        }
        (
            ast::ExpKind::UpCast(typ_template, exp_template_inner),
            ast::ExpKind::UpCast(typ, exp_inner),
        ) if typ_template.syntax_eq(typ) => {
            populate_exp(ids_unifier, exp_template_inner, exp_inner)
        }
        (ast::ExpKind::Tuple(exps_template), ast::ExpKind::Tuple(exps)) => {
            populate_exps(ids_unifier, exps_template.iter(), exps.iter())
        }
        (ast::ExpKind::Case(not_exp_template), ast::ExpKind::Case(not_exp))
            if not_exp_template.eq_shape(not_exp) =>
        {
            let exps_template = not_exp_template.args();
            let exps = not_exp.args();
            populate_exps(ids_unifier, exps_template.into_iter(), exps.into_iter())
        }
        (ast::ExpKind::Str(expfields_template), ast::ExpKind::Str(expfields)) => {
            let exps_template = expfields_template.iter().map(|(_, exp)| exp);
            let exps = expfields.iter().map(|(_, exp)| exp);
            populate_exps(ids_unifier, exps_template, exps)
        }
        _ => {
            let prem = populate_equality_prem(exp_template, exp);
            vec![prem]
        }
    }
}

fn populate_exps<'a>(
    ids_unifier: &IdSet,
    exps_template: impl Iterator<Item = &'a ast::Exp>,
    exps: impl Iterator<Item = &'a ast::Exp>,
) -> Vec<ast::Prem> {
    exps_template
        .zip(exps)
        .flat_map(|(exp_template, exp)| populate_exp(ids_unifier, exp_template, exp))
        .collect()
}

// - Equality premise

fn populate_equality_prem(exp_template: &ast::Exp, exp: &ast::Exp) -> ast::Prem {
    let span = Span::over(&[exp_template.span.clone(), exp.span.clone()]);
    let op = ast::CmpOp::Bool(xl::bool::CmpOp::Eq);
    let exp_template = Box::new(exp_template.clone());
    let exp = Box::new(exp.clone());
    let exp_kind = ast::ExpKind::Cmp(op, ast::OpTyp::Bool, exp_template, exp);
    let exp_match = note_phrase! {
        node: exp_kind,
        note: ast::TypKind::Bool,
        span: span.clone(),
    };
    let if_prem = ast::IfPrem { exp: exp_match };
    let prem_kind = ast::PremKind::If(if_prem);
    phrase!(node: prem_kind, span: span)
}

// - Expressions by rule

fn populate_exps_by_rule(
    ids_unifier: &IdSet,
    exps_template: &[ast::Exp],
    exps_by_rule: &[Vec<ast::Exp>],
) -> Vec<Vec<ast::Prem>> {
    exps_by_rule
        .iter()
        .map(|exps| populate_exps(ids_unifier, exps_template.iter(), exps.iter()))
        .collect()
}

// == Entry point

/// Anti-unifies input paths and returns shared templates plus per-path premises
#[allow(clippy::type_complexity)]
pub fn antiunify(
    ctx: &mut Context,
    exps_by_rule: Vec<Vec<ast::Exp>>,
) -> Result<(Vec<ast::Exp>, Vec<Vec<ast::Prem>>), AlgoError> {
    let mut ids_free = ctx.frees.clone();
    let (ids_unifier, exps_template) =
        overlap_exps_across_rules(&ctx.tdenv, &ctx.menv, &mut ids_free, &exps_by_rule)?;
    let prems_by_rule = populate_exps_by_rule(&ids_unifier, &exps_template, &exps_by_rule);
    ctx.add_frees(&ids_unifier);
    Ok((exps_template, prems_by_rule))
}
