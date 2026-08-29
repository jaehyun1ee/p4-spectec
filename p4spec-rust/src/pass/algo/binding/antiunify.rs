//! Anti-unification of rule input expressions

use crate::{
    lang::{
        common::{
            ds::set::IdSet,
            notation::mixop::Mixop,
            noted::Noted,
            source::{Span, Spanned},
        },
        il::{ast, fresh, var},
        traits::eq::SyntaxEq,
        xl,
    },
    runtime::{
        sta::MEnv,
        types::{TDEnv, equiv_typ},
    },
};

use super::{
    super::{AlgoError, AlgoErrorKind},
    context::Context,
};

fn is_overlap_mismatch(error: &AlgoError) -> bool {
    matches!(
        error.kind,
        AlgoErrorKind::AntiUnification | AlgoErrorKind::ExpressionArityMismatch { .. }
    )
}

fn overlap_exps(
    tdenv: &TDEnv,
    menv: &MEnv,
    frees: &mut IdSet,
    unifiers: &mut IdSet,
    exps_template: &[ast::Exp],
    exps: &[ast::Exp],
) -> Result<Vec<ast::Exp>, AlgoError> {
    if exps_template.len() != exps.len() {
        return Err(AlgoError::new(
            AlgoErrorKind::ExpressionArityMismatch {
                expected: exps_template.len(),
                actual: exps.len(),
            },
            Span::default(),
        ));
    }
    let mut exps_overlapped = Vec::with_capacity(exps_template.len());
    for (exp_template, exp) in exps_template.iter().zip(exps) {
        exps_overlapped.push(overlap_exp(
            tdenv,
            menv,
            frees,
            unifiers,
            exp_template,
            exp,
        )?);
    }
    Ok(exps_overlapped)
}

fn overlap_structure(
    tdenv: &TDEnv,
    menv: &MEnv,
    frees: &mut IdSet,
    unifiers: &mut IdSet,
    exp_template: &ast::Exp,
    exp: &ast::Exp,
) -> Result<Option<ast::Exp>, AlgoError> {
    let span = exp_template.span.clone();
    let note = exp_template.node.note.clone();
    let kind = match (&exp_template.node.kind, &exp.node.kind) {
        (ast::ExpKind::Var(id_template), _) if unifiers.contains(id_template) => {
            return Ok(Some(exp_template.clone()));
        }
        (
            ast::ExpKind::UpCast(typ_template, exp_template_inner),
            ast::ExpKind::UpCast(typ, exp_inner),
        ) if typ_template.syntax_eq(typ) => {
            let exp_template_inner =
                overlap_exp(tdenv, menv, frees, unifiers, exp_template_inner, exp_inner)?;
            ast::ExpKind::UpCast(typ_template.clone(), Box::new(exp_template_inner))
        }
        (ast::ExpKind::Tuple(exps_template), ast::ExpKind::Tuple(exps)) => {
            let exps_template = overlap_exps(tdenv, menv, frees, unifiers, exps_template, exps)?;
            ast::ExpKind::Tuple(exps_template)
        }
        (ast::ExpKind::Case(not_exp_template), ast::ExpKind::Case(not_exp))
            if not_exp_template.to_mixop().syntax_eq(&not_exp.to_mixop()) =>
        {
            let mixop = not_exp_template.to_mixop();
            let exps_template = not_exp_template
                .args()
                .into_iter()
                .cloned()
                .collect::<Vec<_>>();
            let exps = not_exp.args().into_iter().cloned().collect::<Vec<_>>();
            let exps_template = overlap_exps(tdenv, menv, frees, unifiers, &exps_template, &exps)?;
            let not_exp_template = Mixop::fill(&mixop, exps_template)
                .expect("arguments obtained from the same mixfix must match its arity");
            ast::ExpKind::Case(Box::new(not_exp_template))
        }
        (ast::ExpKind::Str(fields_template), ast::ExpKind::Str(fields))
            if fields_template.len() == fields.len()
                && fields_template
                    .iter()
                    .zip(fields)
                    .all(|((atom_template, _), (atom, _))| atom_template.syntax_eq(atom)) =>
        {
            let exps_template = fields_template
                .iter()
                .map(|(_, exp)| exp.clone())
                .collect::<Vec<_>>();
            let exps = fields
                .iter()
                .map(|(_, exp)| exp.clone())
                .collect::<Vec<_>>();
            let exps_template = overlap_exps(tdenv, menv, frees, unifiers, &exps_template, &exps)?;
            let fields_template = fields_template
                .iter()
                .map(|(atom, _)| atom.clone())
                .zip(exps_template)
                .collect();
            ast::ExpKind::Str(fields_template)
        }
        _ => return Ok(None),
    };
    Ok(Some(Spanned::new(Noted::new(kind, note), span)))
}

fn overlap_exp(
    tdenv: &TDEnv,
    menv: &MEnv,
    frees: &mut IdSet,
    unifiers: &mut IdSet,
    exp_template: &ast::Exp,
    exp: &ast::Exp,
) -> Result<ast::Exp, AlgoError> {
    if exp_template.syntax_eq(exp) {
        return Ok(exp_template.clone());
    }

    let mut frees_structural = frees.clone();
    let mut unifiers_structural = unifiers.clone();
    match overlap_structure(
        tdenv,
        menv,
        &mut frees_structural,
        &mut unifiers_structural,
        exp_template,
        exp,
    ) {
        Ok(Some(exp_template)) => {
            *frees = frees_structural;
            *unifiers = unifiers_structural;
            return Ok(exp_template);
        }
        Ok(None) => {}
        Err(error) if is_overlap_mismatch(&error) => {}
        Err(error) => return Err(error),
    }

    let typ_template = Spanned::new(exp_template.node.note.clone(), exp_template.span.clone());
    let typ = Spanned::new(exp.node.note.clone(), exp.span.clone());
    if !equiv_typ(tdenv, &typ_template, &typ)? {
        return Err(AlgoError::new(
            AlgoErrorKind::AntiUnification,
            exp.span.clone(),
        ));
    }
    let var_fresh = fresh::var_from_typ(menv, frees, exp_template.span.clone(), &typ_template);
    frees.insert(var_fresh.id.clone());
    unifiers.insert(var_fresh.id.clone());
    Ok(var::as_exp(true, &var_fresh))
}

fn overlap_exp_group(
    tdenv: &TDEnv,
    menv: &MEnv,
    frees: &mut IdSet,
    exps: &[ast::Exp],
) -> Result<(IdSet, ast::Exp), AlgoError> {
    let Some((exp_template, exps)) = exps.split_first() else {
        return Err(AlgoError::new(
            AlgoErrorKind::ExpressionArityMismatch {
                expected: 1,
                actual: 0,
            },
            Span::default(),
        ));
    };
    let mut unifiers = IdSet::new();
    let mut exp_template = exp_template.clone();
    for exp in exps {
        exp_template = overlap_exp(tdenv, menv, frees, &mut unifiers, &exp_template, exp)?;
    }
    Ok((unifiers, exp_template))
}

fn equality_prem(exp_template: &ast::Exp, exp: &ast::Exp) -> ast::Prem {
    let span = Span::over(&[exp_template.span.clone(), exp.span.clone()]);
    let exp_match = Spanned::new(
        Noted::new(
            ast::ExpKind::Cmp(
                ast::CmpOp::Bool(xl::bool::CmpOp::Eq),
                ast::OpTyp::Bool,
                Box::new(exp_template.clone()),
                Box::new(exp.clone()),
            ),
            ast::TypKind::Bool,
        ),
        span.clone(),
    );
    Spanned::new(ast::PremKind::If(ast::IfPrem { exp: exp_match }), span)
}

fn populate_exps(
    unifiers: &IdSet,
    exps_template: &[ast::Exp],
    exps: &[ast::Exp],
) -> Vec<ast::Prem> {
    exps_template
        .iter()
        .zip(exps)
        .flat_map(|(exp_template, exp)| populate_exp(unifiers, exp_template, exp))
        .collect()
}

fn populate_exp(unifiers: &IdSet, exp_template: &ast::Exp, exp: &ast::Exp) -> Vec<ast::Prem> {
    if exp_template.syntax_eq(exp) {
        return vec![];
    }
    match (&exp_template.node.kind, &exp.node.kind) {
        (ast::ExpKind::Var(id_template), _) if unifiers.contains(id_template) => {
            vec![equality_prem(exp_template, exp)]
        }
        (
            ast::ExpKind::UpCast(typ_template, exp_template_inner),
            ast::ExpKind::UpCast(typ, exp_inner),
        ) if typ_template.syntax_eq(typ) => populate_exp(unifiers, exp_template_inner, exp_inner),
        (ast::ExpKind::Tuple(exps_template), ast::ExpKind::Tuple(exps)) => {
            populate_exps(unifiers, exps_template, exps)
        }
        (ast::ExpKind::Case(not_exp_template), ast::ExpKind::Case(not_exp))
            if not_exp_template.to_mixop().syntax_eq(&not_exp.to_mixop()) =>
        {
            let exps_template = not_exp_template
                .args()
                .into_iter()
                .cloned()
                .collect::<Vec<_>>();
            let exps = not_exp.args().into_iter().cloned().collect::<Vec<_>>();
            populate_exps(unifiers, &exps_template, &exps)
        }
        (ast::ExpKind::Str(fields_template), ast::ExpKind::Str(fields)) => {
            let exps_template = fields_template
                .iter()
                .map(|(_, exp)| exp.clone())
                .collect::<Vec<_>>();
            let exps = fields
                .iter()
                .map(|(_, exp)| exp.clone())
                .collect::<Vec<_>>();
            populate_exps(unifiers, &exps_template, &exps)
        }
        _ => vec![equality_prem(exp_template, exp)],
    }
}

/// Anti-unifies input paths and returns shared templates plus per-path premises
#[allow(clippy::type_complexity)]
pub fn antiunify(
    mut ctx: Context,
    exps_group: Vec<Vec<ast::Exp>>,
) -> Result<(Context, Vec<ast::Exp>, Vec<Vec<ast::Prem>>), AlgoError> {
    let Some(first) = exps_group.first() else {
        return Ok((ctx, vec![], vec![]));
    };
    for exps in &exps_group[1..] {
        if exps.len() != first.len() {
            return Err(AlgoError::new(
                AlgoErrorKind::ExpressionArityMismatch {
                    expected: first.len(),
                    actual: exps.len(),
                },
                Span::default(),
            ));
        }
    }
    if exps_group.len() == 1 {
        return Ok((ctx, first.clone(), vec![vec![]]));
    }

    let mut frees = ctx.frees.clone();
    let mut unifiers = IdSet::new();
    let mut exps_template = Vec::with_capacity(first.len());
    for index in 0..first.len() {
        let exps = exps_group
            .iter()
            .map(|group| group[index].clone())
            .collect::<Vec<_>>();
        let (unifiers_column, exp_template) =
            overlap_exp_group(&ctx.tdenv, &ctx.menv, &mut frees, &exps)?;
        unifiers.extend(unifiers_column.iter().cloned());
        exps_template.push(exp_template);
    }
    let prems_group = exps_group
        .iter()
        .map(|exps| populate_exps(&unifiers, &exps_template, exps))
        .collect();
    ctx.add_frees(&unifiers);
    Ok((ctx, exps_template, prems_group))
}
