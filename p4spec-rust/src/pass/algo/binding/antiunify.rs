//! Anti-unification of rule input expressions

use crate::{
    lang::{
        common::{ds::set::IdSet, notation::mixop::Mixop, source::Span},
        il::{ast, fresh, var},
        traits::eq::SyntaxEq,
        xl,
    },
    note_phrase, phrase,
    runtime::{env::TDEnv, envs::elab::MEnv, ops::typ::equiv_typ},
};

use super::{
    super::{AlgoError, AlgoErrorKind},
    context::Context,
};

// == Helpers

// - Unifier identifiers

#[derive(Clone)]
struct UnifierIds(IdSet);

impl UnifierIds {
    fn new() -> Self {
        Self(IdSet::new())
    }

    fn contains(&self, id: &ast::Id) -> bool {
        self.0.contains(id)
    }

    fn insert(&mut self, ids_free: &mut IdSet, id: ast::Id) {
        ids_free.insert(id.clone());
        self.0.insert(id);
    }

    fn extend(&mut self, ids_other: &Self) {
        self.0.extend(ids_other.0.iter().cloned());
    }

    fn as_ids(&self) -> &IdSet {
        &self.0
    }
}

// - Errors

fn is_overlap_mismatch(error: &AlgoError) -> bool {
    matches!(
        error.kind,
        AlgoErrorKind::AntiUnification | AlgoErrorKind::ExpressionArityMismatch { .. }
    )
}

// == Template overlap

// - Expressions

fn overlap_exp(
    tdenv: &TDEnv,
    menv: &MEnv,
    ids_free: &mut IdSet,
    ids_unifier: &mut UnifierIds,
    exp_template: &ast::Exp,
    exp: &ast::Exp,
) -> Result<ast::Exp, AlgoError> {
    if exp_template.syntax_eq(exp) {
        let exp_template = exp_template.clone();
        return Ok(exp_template);
    }

    let mut ids_free_structural = ids_free.clone();
    let mut ids_unifier_structural = ids_unifier.clone();
    let span = exp_template.span.clone();
    let note = exp_template.note.clone();
    let kind = match (&exp_template.node, &exp.node) {
        (ast::ExpKind::Var(id_template), _) if ids_unifier.contains(id_template) => {
            let kind = exp_template.node.clone();
            Ok(Some(kind))
        }
        (
            ast::ExpKind::UpCast(typ_template, exp_template_inner),
            ast::ExpKind::UpCast(typ, exp_inner),
        ) if typ_template.syntax_eq(typ) => {
            let exp_template_inner = overlap_exp(
                tdenv,
                menv,
                &mut ids_free_structural,
                &mut ids_unifier_structural,
                exp_template_inner,
                exp_inner,
            );
            match exp_template_inner {
                Ok(exp_template_inner) => {
                    let exp_template_inner = Box::new(exp_template_inner);
                    let kind = ast::ExpKind::UpCast(typ_template.clone(), exp_template_inner);
                    Ok(Some(kind))
                }
                Err(error) => Err(error),
            }
        }
        (ast::ExpKind::Tuple(exps_template), ast::ExpKind::Tuple(exps)) => {
            let exps_template = overlap_exps(
                tdenv,
                menv,
                &mut ids_free_structural,
                &mut ids_unifier_structural,
                exps_template,
                exps,
            );
            match exps_template {
                Ok(exps_template) => {
                    let kind = ast::ExpKind::Tuple(exps_template);
                    Ok(Some(kind))
                }
                Err(error) => Err(error),
            }
        }
        (ast::ExpKind::Case(not_exp_template), ast::ExpKind::Case(not_exp))
            if not_exp_template.eq_shape(not_exp) =>
        {
            let mixop = not_exp_template.to_mixop();
            let exps_template = not_exp_template
                .args()
                .into_iter()
                .cloned()
                .collect::<Vec<_>>();
            let exps = not_exp.args().into_iter().cloned().collect::<Vec<_>>();
            let exps_template = overlap_exps(
                tdenv,
                menv,
                &mut ids_free_structural,
                &mut ids_unifier_structural,
                &exps_template,
                &exps,
            );
            match exps_template {
                Ok(exps_template) => {
                    let not_exp_template = Mixop::fill(&mixop, exps_template);
                    let not_exp_template = not_exp_template
                        .expect("overlapped arguments must preserve the template mixfix arity");
                    let not_exp_template = Box::new(not_exp_template);
                    let kind = ast::ExpKind::Case(not_exp_template);
                    Ok(Some(kind))
                }
                Err(error) => Err(error),
            }
        }
        (ast::ExpKind::Str(exp_fields_template), ast::ExpKind::Str(exp_fields))
            if exp_fields_template.len() == exp_fields.len()
                && exp_fields_template
                    .iter()
                    .zip(exp_fields)
                    .all(|((atom_template, _), (atom, _))| atom_template.syntax_eq(atom)) =>
        {
            let exps_template = exp_fields_template
                .iter()
                .map(|(_, exp)| exp.clone())
                .collect::<Vec<_>>();
            let exps = exp_fields
                .iter()
                .map(|(_, exp)| exp.clone())
                .collect::<Vec<_>>();
            let exps_template = overlap_exps(
                tdenv,
                menv,
                &mut ids_free_structural,
                &mut ids_unifier_structural,
                &exps_template,
                &exps,
            );
            match exps_template {
                Ok(exps_template) => {
                    let exp_fields_template = exp_fields_template
                        .iter()
                        .map(|(atom, _)| atom.clone())
                        .zip(exps_template)
                        .collect();
                    let kind = ast::ExpKind::Str(exp_fields_template);
                    Ok(Some(kind))
                }
                Err(error) => Err(error),
            }
        }
        _ => Ok(None),
    };
    let overlap = match kind {
        Ok(Some(kind)) => {
            let exp_template = note_phrase!(node: kind, note: note, span: span);
            Ok(Some(exp_template))
        }
        Ok(None) => Ok(None),
        Err(error) => Err(error),
    };
    match overlap {
        Ok(Some(exp_template)) => {
            *ids_free = ids_free_structural;
            *ids_unifier = ids_unifier_structural;
            return Ok(exp_template);
        }
        Ok(None) => {}
        Err(error) if is_overlap_mismatch(&error) => {}
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
    ids_unifier.insert(ids_free, var_fresh.id.clone());
    let exp_template = var::as_exp(true, &var_fresh);
    Ok(exp_template)
}

fn overlap_exps(
    tdenv: &TDEnv,
    menv: &MEnv,
    ids_free: &mut IdSet,
    ids_unifier: &mut UnifierIds,
    exps_template: &[ast::Exp],
    exps: &[ast::Exp],
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
    for (exp_template, exp) in exps_template.iter().zip(exps) {
        let exp_overlapped = overlap_exp(tdenv, menv, ids_free, ids_unifier, exp_template, exp)?;
        exps_overlapped.push(exp_overlapped);
    }
    Ok(exps_overlapped)
}

// - Expression groups

fn overlap_exp_group(
    tdenv: &TDEnv,
    menv: &MEnv,
    ids_free: &mut IdSet,
    exps: &[ast::Exp],
) -> Result<(UnifierIds, ast::Exp), AlgoError> {
    let Some((exp_template, exps)) = exps.split_first() else {
        let kind = AlgoErrorKind::ExpressionArityMismatch {
            expected: 1,
            actual: 0,
        };
        let error = AlgoError::new(kind, Span::default());
        return Err(error);
    };
    let mut ids_unifier = UnifierIds::new();
    let mut exp_template = exp_template.clone();
    for exp in exps {
        exp_template = overlap_exp(tdenv, menv, ids_free, &mut ids_unifier, &exp_template, exp)?;
    }
    Ok((ids_unifier, exp_template))
}

fn overlap_exps_group(
    tdenv: &TDEnv,
    menv: &MEnv,
    ids_free: &mut IdSet,
    exps_group: &[Vec<ast::Exp>],
) -> Result<(UnifierIds, Vec<ast::Exp>), AlgoError> {
    let Some(exps_first) = exps_group.first() else {
        let ids_unifier = UnifierIds::new();
        let exps_template = Vec::new();
        return Ok((ids_unifier, exps_template));
    };
    for exps in &exps_group[1..] {
        if exps.len() != exps_first.len() {
            let kind = AlgoErrorKind::ExpressionArityMismatch {
                expected: exps_first.len(),
                actual: exps.len(),
            };
            let error = AlgoError::new(kind, Span::default());
            return Err(error);
        }
    }
    if exps_group.len() == 1 {
        let ids_unifier = UnifierIds::new();
        let exps_template = exps_first.clone();
        return Ok((ids_unifier, exps_template));
    }

    let mut ids_unifier = UnifierIds::new();
    let mut exps_template = Vec::with_capacity(exps_first.len());
    for index in 0..exps_first.len() {
        let exps_column = exps_group
            .iter()
            .map(|exps| exps[index].clone())
            .collect::<Vec<_>>();
        let (ids_unifier_column, exp_template) =
            overlap_exp_group(tdenv, menv, ids_free, &exps_column)?;
        ids_unifier.extend(&ids_unifier_column);
        exps_template.push(exp_template);
    }
    Ok((ids_unifier, exps_template))
}

// == Template population

// - Premises

fn equality_prem(exp_template: &ast::Exp, exp: &ast::Exp) -> ast::Prem {
    let span = Span::over(&[exp_template.span.clone(), exp.span.clone()]);
    let exp_match = note_phrase! {
        node: ast::ExpKind::Cmp(
            ast::CmpOp::Bool(xl::bool::CmpOp::Eq),
            ast::OpTyp::Bool,
            Box::new(exp_template.clone()),
            Box::new(exp.clone()),
        ),
        note: ast::TypKind::Bool,
        span: span.clone(),
    };
    let if_prem = ast::IfPrem { exp: exp_match };
    let prem_kind = ast::PremKind::If(if_prem);
    phrase!(node: prem_kind, span: span)
}

fn populate_exps(
    ids_unifier: &UnifierIds,
    exps_template: &[ast::Exp],
    exps: &[ast::Exp],
) -> Vec<ast::Prem> {
    exps_template
        .iter()
        .zip(exps)
        .flat_map(|(exp_template, exp)| populate_exp(ids_unifier, exp_template, exp))
        .collect()
}

fn populate_exp(
    ids_unifier: &UnifierIds,
    exp_template: &ast::Exp,
    exp: &ast::Exp,
) -> Vec<ast::Prem> {
    if exp_template.syntax_eq(exp) {
        return vec![];
    }
    match (&exp_template.node, &exp.node) {
        (ast::ExpKind::Var(id_template), _) if ids_unifier.contains(id_template) => {
            let prem = equality_prem(exp_template, exp);
            vec![prem]
        }
        (
            ast::ExpKind::UpCast(typ_template, exp_template_inner),
            ast::ExpKind::UpCast(typ, exp_inner),
        ) if typ_template.syntax_eq(typ) => {
            populate_exp(ids_unifier, exp_template_inner, exp_inner)
        }
        (ast::ExpKind::Tuple(exps_template), ast::ExpKind::Tuple(exps)) => {
            populate_exps(ids_unifier, exps_template, exps)
        }
        (ast::ExpKind::Case(not_exp_template), ast::ExpKind::Case(not_exp))
            if not_exp_template.eq_shape(not_exp) =>
        {
            let exps_template = not_exp_template
                .args()
                .into_iter()
                .cloned()
                .collect::<Vec<_>>();
            let exps = not_exp.args().into_iter().cloned().collect::<Vec<_>>();
            populate_exps(ids_unifier, &exps_template, &exps)
        }
        (ast::ExpKind::Str(exp_fields_template), ast::ExpKind::Str(exp_fields)) => {
            let exps_template = exp_fields_template
                .iter()
                .map(|(_, exp)| exp.clone())
                .collect::<Vec<_>>();
            let exps = exp_fields
                .iter()
                .map(|(_, exp)| exp.clone())
                .collect::<Vec<_>>();
            populate_exps(ids_unifier, &exps_template, &exps)
        }
        _ => {
            let prem = equality_prem(exp_template, exp);
            vec![prem]
        }
    }
}

// - Expression groups

fn populate_exps_group(
    ids_unifier: &UnifierIds,
    exps_template: &[ast::Exp],
    exps_group: &[Vec<ast::Exp>],
) -> Vec<Vec<ast::Prem>> {
    exps_group
        .iter()
        .map(|exps| populate_exps(ids_unifier, exps_template, exps))
        .collect()
}

// == Entry point

/// Anti-unifies input paths and returns shared templates plus per-path premises
#[allow(clippy::type_complexity)]
pub fn antiunify(
    ctx: &mut Context,
    exps_group: Vec<Vec<ast::Exp>>,
) -> Result<(Vec<ast::Exp>, Vec<Vec<ast::Prem>>), AlgoError> {
    let mut ids_free = ctx.frees.clone();
    let (ids_unifier, exps_template) =
        overlap_exps_group(&ctx.tdenv, &ctx.menv, &mut ids_free, &exps_group)?;
    let prems_group = populate_exps_group(&ids_unifier, &exps_template, &exps_group);
    ctx.add_frees(ids_unifier.as_ids());
    Ok((exps_template, prems_group))
}
