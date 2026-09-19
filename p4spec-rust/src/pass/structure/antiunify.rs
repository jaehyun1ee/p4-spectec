//! Find shared input templates, then bind each original input to its template
//!
//! Inputs `(x, true)` and `(false, true)` share `(x', true)`
//! Their paths start with `let x = x'` and `let false = x'`, respectively
//!
//! Inputs -> shared template -> binding premises prepended to each path

use crate::lang::{
    al::ast::*,
    common::{
        ds::{map::IdMap, set::IdSet},
        source::Span,
    },
    il,
    traits::{eq::SyntaxEq, free::Free},
};

use super::{StructureError, StructureErrorKind};

// == Unification environment

// Maps original identifiers to their unified identifiers
#[derive(Default)]
struct UEnv {
    ids: IdMap<Id>,
}

impl UEnv {
    fn unified(&self, id: &Id) -> bool {
        self.ids
            .iter()
            .any(|(_, id_unified)| id_unified.syntax_eq(id))
    }

    fn extend(&mut self, uenv: Self) -> Result<(), StructureError> {
        for (id, id_unified) in uenv.ids.iter() {
            if self.ids.contains_key(id) {
                let error_kind = StructureErrorKind::ConflictingUnification;
                let error = StructureError::new(error_kind, id.span.clone());
                return Err(error);
            }
            self.ids.insert(id.clone(), id_unified.clone());
        }
        Ok(())
    }
}

// == Arity checks

fn check_arity(expected: usize, actual: usize, span: &Span) -> Result<(), StructureError> {
    if expected == actual {
        Ok(())
    } else {
        let error_kind = StructureErrorKind::ArityMismatch { expected, actual };
        let error = StructureError::new(error_kind, span.clone());
        Err(error)
    }
}

// == Populating expression templates

// - Expression template

fn populate_exp_template(
    uenv: &UEnv,
    exp_template: &Exp,
    exp: &Exp,
) -> Result<Vec<Prem>, StructureError> {
    if exp_template.syntax_eq(exp) {
        return Ok(vec![]);
    }
    match (&exp_template.node, &exp.node) {
        (ExpKind::Id(id_template), _) if uenv.unified(id_template) => {
            let prem = populate_id_exp_template(exp_template, exp);
            Ok(vec![prem])
        }
        (ExpKind::Tuple(exps_template), ExpKind::Tuple(exps)) => {
            populate_exps_templates(uenv, exps_template.iter(), exps.iter(), &exp.span)
        }
        (ExpKind::Case(not_exp_template), ExpKind::Case(not_exp))
            if not_exp_template.eq_shape(not_exp) =>
        {
            let exps_template = not_exp_template.args();
            let exps = not_exp.args();
            populate_exps_templates(uenv, exps_template.into_iter(), exps.into_iter(), &exp.span)
        }
        (ExpKind::Str(expfields_template), ExpKind::Str(expfields)) => {
            let exps_template = expfields_template.iter().map(|(_, exp)| exp);
            let exps = expfields.iter().map(|(_, exp)| exp);
            populate_exps_templates(uenv, exps_template, exps, &exp.span)
        }
        (ExpKind::Iter(exp_body_template, iter_template), ExpKind::Iter(exp_body, iter))
            if iter_template.0.syntax_eq(&iter.0) =>
        {
            let prem = populate_iter_exp_template(exp_body_template, iter_template, exp_body, iter);
            Ok(vec![prem])
        }
        _ => {
            let error_kind = StructureErrorKind::TemplatePopulation;
            let error = StructureError::new(error_kind, exp.span.clone());
            Err(error)
        }
    }
}

fn populate_exps_templates<'a>(
    uenv: &UEnv,
    exps_template: impl ExactSizeIterator<Item = &'a Exp>,
    exps: impl ExactSizeIterator<Item = &'a Exp>,
    span: &Span,
) -> Result<Vec<Prem>, StructureError> {
    check_arity(exps_template.len(), exps.len(), span)?;
    let mut prems = vec![];
    for (exp_template, exp) in exps_template.zip(exps) {
        let prems_exp = populate_exp_template(uenv, exp_template, exp)?;
        prems.extend(prems_exp);
    }
    Ok(prems)
}

// - Identifier expression

fn populate_id_exp_template(exp_template: &Exp, exp: &Exp) -> Prem {
    let span = Span::over(&[exp.span.clone(), exp_template.span.clone()]);
    let prem = LetPrem { exp_l: exp.clone(), exp_r: exp_template.clone() };
    let prem_kind = PremKind::Let(prem);
    crate::phrase! {node: prem_kind, span: span}
}

// - Iterated expression

fn populate_iter_exp_template(
    exp_template: &Exp,
    iter_template: &ExpIter,
    exp: &Exp,
    iter: &ExpIter,
) -> Prem {
    let (iter_template, vars_template) = iter_template;
    let (_, vars) = iter;
    let prem = populate_id_exp_template(exp_template, exp);
    let span = prem.span.clone();
    let prem_iter = PremIter {
        iter: *iter_template,
        vars_bound: vars_template.clone(),
        vars_bind: vars.clone(),
    };
    let prem = Box::new(prem);
    let prem = IterPrem { prem, prem_iter };
    let prem_kind = PremKind::Iter(prem);
    crate::phrase! {node: prem_kind, span: span}
}

// == Anti-unification of expressions

// - Expression

fn antiunify_exp(
    frees: &mut IdSet,
    uenv: &mut UEnv,
    exp_template: &Exp,
    exp: &Exp,
) -> Result<Exp, StructureError> {
    if exp_template.syntax_eq(exp) {
        return Ok(exp_template.clone());
    }
    let exp_kind_template = match (&exp_template.node, &exp.node) {
        (ExpKind::Id(id_template), _) => antiunify_id_exp(frees, uenv, id_template),
        (_, ExpKind::Id(id)) => antiunify_fresh_id_exp(frees, uenv, id),
        (ExpKind::Tuple(exps_template), ExpKind::Tuple(exps)) => {
            let exps_template = antiunify_exps(frees, uenv, exps_template, exps, &exp.span)?;
            ExpKind::Tuple(exps_template)
        }
        (ExpKind::Case(not_exp_template), ExpKind::Case(not_exp))
            if not_exp_template.eq_shape(not_exp) =>
        {
            antiunify_case_exp(frees, uenv, not_exp_template, not_exp)?
        }
        (ExpKind::Str(expfields_template), ExpKind::Str(expfields)) => {
            antiunify_str_exp(frees, uenv, expfields_template, expfields, &exp.span)?
        }
        (ExpKind::Iter(exp_body_template, iter_template), ExpKind::Iter(exp_body, iter))
            if iter_template.0.syntax_eq(&iter.0) =>
        {
            antiunify_iter_exp(frees, uenv, exp_body_template, iter_template, exp_body, iter)?
        }
        _ => {
            let error_kind = StructureErrorKind::Antiunification;
            let error = StructureError::new(error_kind, exp.span.clone());
            return Err(error);
        }
    };
    let exp_template = crate::note_phrase! {
        node: exp_kind_template,
        note: exp_template.note.clone(),
        span: exp_template.span.clone()
    };
    Ok(exp_template)
}

fn antiunify_exps(
    frees: &mut IdSet,
    uenv: &mut UEnv,
    exps_template: &[Exp],
    exps: &[Exp],
    span: &Span,
) -> Result<Vec<Exp>, StructureError> {
    check_arity(exps_template.len(), exps.len(), span)?;
    exps_template
        .iter()
        .zip(exps)
        .map(|(exp_template, exp)| antiunify_exp(frees, uenv, exp_template, exp))
        .collect()
}

// - Identifier expression

fn antiunify_id_exp(frees: &mut IdSet, uenv: &mut UEnv, id_template: &Id) -> ExpKind {
    if uenv.unified(id_template) {
        uenv.ids.insert(id_template.clone(), id_template.clone());
        ExpKind::Id(id_template.clone())
    } else {
        antiunify_fresh_id_exp(frees, uenv, id_template)
    }
}

// - Fresh identifier expression

fn antiunify_fresh_id_exp(frees: &mut IdSet, uenv: &mut UEnv, id: &Id) -> ExpKind {
    let id_fresh = il::fresh::id(frees, id);
    frees.insert(id_fresh.clone());
    uenv.ids.insert(id.clone(), id_fresh.clone());
    ExpKind::Id(id_fresh)
}

// - Case expression

fn antiunify_case_exp(
    frees: &mut IdSet,
    uenv: &mut UEnv,
    not_exp_template: &NotExp,
    not_exp: &NotExp,
) -> Result<ExpKind, StructureError> {
    let (mixop, exps_template) = not_exp_template.split();
    let exps = not_exp.args();
    let mut exps_unified = vec![];
    for (exp_template, exp) in exps_template.iter().zip(exps) {
        let exp_unified = antiunify_exp(frees, uenv, exp_template, exp)?;
        exps_unified.push(exp_unified);
    }
    let not_exp_template = Mixop::fill(&mixop, exps_unified)
        .expect("matching mixfix shapes have equal argument counts");
    let not_exp_template = Box::new(not_exp_template);
    Ok(ExpKind::Case(not_exp_template))
}

// - Record expression

fn antiunify_str_exp(
    frees: &mut IdSet,
    uenv: &mut UEnv,
    expfields_template: &[ExpField],
    expfields: &[ExpField],
    span: &Span,
) -> Result<ExpKind, StructureError> {
    check_arity(expfields_template.len(), expfields.len(), span)?;
    if !expfields_template
        .iter()
        .zip(expfields)
        .all(|((atom_template, _), (atom, _))| atom_template.syntax_eq(atom))
    {
        let error_kind = StructureErrorKind::Antiunification;
        let error = StructureError::new(error_kind, span.clone());
        return Err(error);
    }
    let mut expfields_unified = vec![];
    for ((atom_template, exp_template), (_, exp)) in expfields_template.iter().zip(expfields) {
        let exp_template = antiunify_exp(frees, uenv, exp_template, exp)?;
        expfields_unified.push((atom_template.clone(), exp_template));
    }
    Ok(ExpKind::Str(expfields_unified))
}

// - Iterated expression

fn antiunify_iter_exp(
    frees: &mut IdSet,
    uenv: &mut UEnv,
    exp_template: &Exp,
    iter_template: &ExpIter,
    exp: &Exp,
    iter: &ExpIter,
) -> Result<ExpKind, StructureError> {
    let (iter_template, vars_template) = iter_template;
    let (_, vars) = iter;
    let exp_template = antiunify_exp(frees, uenv, exp_template, exp)?;
    let mut vars_unified = vec![];
    for var in vars_template.iter().chain(vars) {
        let Var { id, typ, iters } = var;
        if let Some(id_unified) = uenv.ids.get(id) {
            let var_unified =
                Var { id: id_unified.clone(), typ: typ.clone(), iters: iters.clone() };
            if !vars_unified
                .iter()
                .any(|var: &Var| var.syntax_eq(&var_unified))
            {
                vars_unified.push(var_unified);
            }
        }
    }
    let exp_template = Box::new(exp_template);
    let iter = (*iter_template, vars_unified);
    Ok(ExpKind::Iter(exp_template, iter))
}

// - Expressions across matches

fn antiunify_exps_across_matches(
    mut frees: IdSet,
    exps_by_match: &[&[Exp]],
) -> Result<(UEnv, Vec<Exp>), StructureError> {
    let Some((exps_head, exps_tail)) = exps_by_match.split_first() else {
        return Ok((UEnv::default(), vec![]));
    };
    for exps in exps_tail {
        let spans = exps.iter().map(|exp| exp.span.clone()).collect::<Vec<_>>();
        let span = Span::over(&spans);
        check_arity(exps_head.len(), exps.len(), &span)?;
    }
    let mut uenv_acc = UEnv::default();
    let mut exps_template = vec![];
    // Share fresh names across input positions; keep a separate map for each
    for (num_idx, exp_head) in exps_head.iter().enumerate() {
        let mut uenv = UEnv::default();
        let mut exp_template = exp_head.clone();
        for exps in exps_tail {
            exp_template = antiunify_exp(&mut frees, &mut uenv, &exp_template, &exps[num_idx])?;
        }
        uenv_acc.extend(uenv)?;
        exps_template.push(exp_template);
    }
    Ok((uenv_acc, exps_template))
}

// == Populating argument templates

// - Argument template

fn populate_arg_template(
    uenv: &UEnv,
    arg_template: &Arg,
    arg: &Arg,
) -> Result<Vec<Prem>, StructureError> {
    match (&arg_template.node, &arg.node) {
        (ArgKind::Exp(exp_template), ArgKind::Exp(exp)) => {
            populate_exp_template(uenv, exp_template, exp)
        }
        (ArgKind::Def(id_template), ArgKind::Def(id)) if id_template.syntax_eq(id) => Ok(vec![]),
        _ => {
            let error_kind = StructureErrorKind::IncompatibleArguments;
            let error = StructureError::new(error_kind, arg.span.clone());
            Err(error)
        }
    }
}

fn populate_args_templates(
    uenv: &UEnv,
    args_template: &[Arg],
    args: &[Arg],
    span: &Span,
) -> Result<Vec<Prem>, StructureError> {
    check_arity(args_template.len(), args.len(), span)?;
    let mut prems = vec![];
    for (arg_template, arg) in args_template.iter().zip(args) {
        let prems_arg = populate_arg_template(uenv, arg_template, arg)?;
        prems.extend(prems_arg);
    }
    Ok(prems)
}

// == Anti-unification of arguments

// - Argument

fn antiunify_arg(
    frees: &mut IdSet,
    uenv: &mut UEnv,
    arg_template: &Arg,
    arg: &Arg,
) -> Result<Arg, StructureError> {
    match (&arg_template.node, &arg.node) {
        (ArgKind::Exp(exp_template), ArgKind::Exp(exp)) => {
            let exp_template = antiunify_exp(frees, uenv, exp_template, exp)?;
            let exp_template = Box::new(exp_template);
            let arg_kind_template = ArgKind::Exp(exp_template);
            let arg_template =
                crate::phrase! {node: arg_kind_template, span: arg_template.span.clone()};
            Ok(arg_template)
        }
        (ArgKind::Def(id_template), ArgKind::Def(id)) if id_template.syntax_eq(id) => {
            Ok(arg_template.clone())
        }
        _ => {
            let error_kind = StructureErrorKind::IncompatibleArguments;
            let error = StructureError::new(error_kind, arg.span.clone());
            Err(error)
        }
    }
}

// - Arguments across clauses

fn antiunify_args_across_clauses(
    mut frees: IdSet,
    clauses: &[&Clause],
) -> Result<(UEnv, Vec<Arg>), StructureError> {
    let Some((clause_head, clauses_tail)) = clauses.split_first() else {
        return Ok((UEnv::default(), vec![]));
    };
    let args_head = &clause_head.node.args;
    for clause in clauses_tail {
        check_arity(args_head.len(), clause.node.args.len(), &clause.span)?;
    }
    let mut uenv_acc = UEnv::default();
    let mut args_template = vec![];
    // Share fresh names across input positions; keep a separate map for each
    for (num_idx, arg_head) in args_head.iter().enumerate() {
        let mut uenv = UEnv::default();
        let mut arg_template = arg_head.clone();
        for clause in clauses_tail {
            arg_template =
                antiunify_arg(&mut frees, &mut uenv, &arg_template, &clause.node.args[num_idx])?;
        }
        uenv_acc.extend(uenv)?;
        args_template.push(arg_template);
    }
    Ok((uenv_acc, args_template))
}

// == Anti-unification of rule matches

#[expect(
    clippy::type_complexity,
    reason = "Destructured once per call site; a named result type would add no meaning"
)]
pub(super) fn antiunify_rule_matches(
    frees: IdSet,
    exps_by_rule_group: &[Vec<Exp>],
    exps_else: Option<&[Exp]>,
) -> Result<(Vec<Exp>, Vec<Vec<Prem>>, Option<Vec<Prem>>), StructureError> {
    let exps_by_match = exps_by_rule_group
        .iter()
        .map(Vec::as_slice)
        .chain(exps_else)
        .collect::<Vec<_>>();
    let (uenv, exps_template) = antiunify_exps_across_matches(frees, &exps_by_match)?;
    let prems_by_rule_group = exps_by_rule_group
        .iter()
        .map(|exps| {
            let spans = exps.iter().map(|exp| exp.span.clone()).collect::<Vec<_>>();
            let span = Span::over(&spans);
            populate_exps_templates(&uenv, exps_template.iter(), exps.iter(), &span)
        })
        .collect::<Result<_, _>>()?;
    let prems_else = exps_else
        .map(|exps| {
            let spans = exps.iter().map(|exp| exp.span.clone()).collect::<Vec<_>>();
            let span = Span::over(&spans);
            populate_exps_templates(&uenv, exps_template.iter(), exps.iter(), &span)
        })
        .transpose()?;
    Ok((exps_template, prems_by_rule_group, prems_else))
}

// == Anti-unification of clauses

fn populate_clause(
    uenv: &UEnv,
    args_template: &[Arg],
    clause: Clause,
) -> Result<(Vec<Prem>, Exp), StructureError> {
    let clause_kind = clause.node;
    let ClauseKind { args, exp, prems } = clause_kind;
    let mut prems_template = populate_args_templates(uenv, args_template, &args, &clause.span)?;
    prems_template.extend(prems);
    Ok((prems_template, exp))
}

#[expect(
    clippy::type_complexity,
    reason = "Destructured once per call site; a named result type would add no meaning"
)]
pub(super) fn antiunify_clauses(
    clauses: Vec<Clause>,
    clause_else: Option<Clause>,
) -> Result<(Vec<Arg>, Vec<(Vec<Prem>, Exp)>, Option<(Vec<Prem>, Exp)>), StructureError> {
    let clauses_all = clauses.iter().chain(clause_else.iter()).collect::<Vec<_>>();
    let mut frees = IdSet::new();
    for clause in &clauses_all {
        clause.free_into(&mut frees);
    }
    let (uenv, args_template) = antiunify_args_across_clauses(frees, &clauses_all)?;
    let paths = clauses
        .into_iter()
        .map(|clause| populate_clause(&uenv, &args_template, clause))
        .collect::<Result<_, _>>()?;
    let path_else = clause_else
        .map(|clause| populate_clause(&uenv, &args_template, clause))
        .transpose()?;
    Ok((args_template, paths, path_else))
}
