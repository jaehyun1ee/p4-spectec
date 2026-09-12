//! Shared input templates and the premises that match each original input

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

// Unification maps original identifiers to their unified identifiers
#[derive(Default)]
struct UEnv {
    ids: IdMap<Id>,
}

impl UEnv {
    fn unified(&self, id: &Id) -> bool {
        self.ids.values().any(|id_unified| id_unified.syntax_eq(id))
    }

    fn extend(&mut self, uenv: Self) -> Result<(), StructureError> {
        for (id, id_unified) in uenv.ids.iter() {
            if self.ids.contains_key(id) {
                return Err(StructureError::new(
                    StructureErrorKind::ConflictingUnification,
                    id.span.clone(),
                ));
            }
            self.ids.insert(id.clone(), id_unified.clone());
        }
        Ok(())
    }
}

fn check_arity(expected: usize, actual: usize, span: &Span) -> Result<(), StructureError> {
    if expected == actual {
        Ok(())
    } else {
        Err(StructureError::new(
            StructureErrorKind::ArityMismatch { expected, actual },
            span.clone(),
        ))
    }
}

// Populating expression templates

fn populate_exp_template(
    uenv: &UEnv,
    exp_template: &Exp,
    exp: &Exp,
) -> Result<Vec<Prem>, StructureError> {
    if exp_template.syntax_eq(exp) {
        return Ok(vec![]);
    }
    match (&exp_template.node, &exp.node) {
        (ExpKind::Var(id_template), _) if uenv.unified(id_template) => {
            Ok(vec![populate_var_exp_template(exp_template, exp)])
        }
        (ExpKind::Tuple(exps_template), ExpKind::Tuple(exps)) => {
            populate_exps_templates(uenv, exps_template, exps, &exp.span)
        }
        (ExpKind::Case(not_exp_template), ExpKind::Case(not_exp))
            if not_exp_template.eq_shape(not_exp) =>
        {
            populate_case_exp_template(uenv, not_exp_template, not_exp, &exp.span)
        }
        (ExpKind::Str(expfields_template), ExpKind::Str(expfields)) => {
            populate_str_exp_template(uenv, expfields_template, expfields, &exp.span)
        }
        (ExpKind::Iter(exp_body_template, iter_template), ExpKind::Iter(exp_body, iter))
            if iter_template.0.syntax_eq(&iter.0) =>
        {
            Ok(vec![populate_iter_exp_template(
                exp_body_template,
                iter_template,
                exp_body,
                iter,
            )])
        }
        _ => Err(StructureError::new(
            StructureErrorKind::TemplatePopulation,
            exp.span.clone(),
        )),
    }
}

fn populate_var_exp_template(exp_template: &Exp, exp: &Exp) -> Prem {
    let span = Span::over(&[exp.span.clone(), exp_template.span.clone()]);
    crate::phrase! {node: PremKind::Let(LetPrem {exp_l: exp.clone(), exp_r: exp_template.clone()}), span: span}
}

fn populate_case_exp_template(
    uenv: &UEnv,
    not_exp_template: &NotExp,
    not_exp: &NotExp,
    span: &Span,
) -> Result<Vec<Prem>, StructureError> {
    populate_exp_refs_templates(uenv, &not_exp_template.args(), &not_exp.args(), span)
}

fn populate_str_exp_template(
    uenv: &UEnv,
    expfields_template: &[ExpField],
    expfields: &[ExpField],
    span: &Span,
) -> Result<Vec<Prem>, StructureError> {
    let exps_template = expfields_template
        .iter()
        .map(|(_, exp)| exp)
        .collect::<Vec<_>>();
    let exps = expfields.iter().map(|(_, exp)| exp).collect::<Vec<_>>();
    populate_exp_refs_templates(uenv, &exps_template, &exps, span)
}

fn populate_iter_exp_template(
    exp_template: &Exp,
    iter_template: &ExpIter,
    exp: &Exp,
    iter: &ExpIter,
) -> Prem {
    let (iter_template, vars_template) = iter_template;
    let (_, vars) = iter;
    let prem = populate_var_exp_template(exp_template, exp);
    let span = prem.span.clone();
    let prem_iter = PremIter {
        iter: *iter_template,
        vars_bound: vars_template.clone(),
        vars_bind: vars.clone(),
    };
    crate::phrase! {node: PremKind::Iter(IterPrem {prem: Box::new(prem), prem_iter}), span: span}
}

fn populate_exp_refs_templates(
    uenv: &UEnv,
    exps_template: &[&Exp],
    exps: &[&Exp],
    span: &Span,
) -> Result<Vec<Prem>, StructureError> {
    check_arity(exps_template.len(), exps.len(), span)?;
    let mut prems = vec![];
    for (exp_template, exp) in exps_template.iter().zip(exps) {
        prems.extend(populate_exp_template(uenv, exp_template, exp)?);
    }
    Ok(prems)
}

fn populate_exps_templates(
    uenv: &UEnv,
    exps_template: &[Exp],
    exps: &[Exp],
    span: &Span,
) -> Result<Vec<Prem>, StructureError> {
    let exps_template = exps_template.iter().collect::<Vec<_>>();
    let exps = exps.iter().collect::<Vec<_>>();
    populate_exp_refs_templates(uenv, &exps_template, &exps, span)
}

// Anti-unification of expressions

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
        (ExpKind::Var(id_template), _) => antiunify_var_exp(frees, uenv, id_template),
        (_, ExpKind::Var(id)) => antiunify_fresh_var_exp(frees, uenv, id),
        (ExpKind::Tuple(exps_template), ExpKind::Tuple(exps)) => {
            ExpKind::Tuple(antiunify_exps(frees, uenv, exps_template, exps, &exp.span)?)
        }
        (ExpKind::Case(not_exp_template), ExpKind::Case(not_exp))
            if not_exp_template.eq_shape(not_exp) =>
        {
            antiunify_case_exp(frees, uenv, not_exp_template, not_exp, &exp.span)?
        }
        (ExpKind::Str(expfields_template), ExpKind::Str(expfields)) => {
            antiunify_str_exp(frees, uenv, expfields_template, expfields, &exp.span)?
        }
        (ExpKind::Iter(exp_body_template, iter_template), ExpKind::Iter(exp_body, iter))
            if iter_template.0.syntax_eq(&iter.0) =>
        {
            antiunify_iter_exp(
                frees,
                uenv,
                exp_body_template,
                iter_template,
                exp_body,
                iter,
            )?
        }
        _ => {
            return Err(StructureError::new(
                StructureErrorKind::Antiunification,
                exp.span.clone(),
            ));
        }
    };
    Ok(
        crate::note_phrase! {node: exp_kind_template, note: exp_template.note.clone(), span: exp_template.span.clone()},
    )
}

fn antiunify_var_exp(frees: &mut IdSet, uenv: &mut UEnv, id_template: &Id) -> ExpKind {
    if uenv.unified(id_template) {
        uenv.ids.insert(id_template.clone(), id_template.clone());
        ExpKind::Var(id_template.clone())
    } else {
        antiunify_fresh_var_exp(frees, uenv, id_template)
    }
}

fn antiunify_fresh_var_exp(frees: &mut IdSet, uenv: &mut UEnv, id: &Id) -> ExpKind {
    let id_fresh = il::fresh::id(frees, id);
    frees.insert(id_fresh.clone());
    uenv.ids.insert(id.clone(), id_fresh.clone());
    ExpKind::Var(id_fresh)
}

fn antiunify_case_exp(
    frees: &mut IdSet,
    uenv: &mut UEnv,
    not_exp_template: &NotExp,
    not_exp: &NotExp,
    span: &Span,
) -> Result<ExpKind, StructureError> {
    let exps_template = not_exp_template.args();
    let exps = not_exp.args();
    check_arity(exps_template.len(), exps.len(), span)?;
    let mut exps_unified = vec![];
    for (exp_template, exp) in exps_template.iter().zip(exps) {
        exps_unified.push(antiunify_exp(frees, uenv, exp_template, exp)?);
    }
    let mut exp_idx = 0;
    let not_exp_template = not_exp_template.map(|_| {
        let exp = exps_unified[exp_idx].clone();
        exp_idx += 1;
        exp
    });
    Ok(ExpKind::Case(Box::new(not_exp_template)))
}

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
        return Err(StructureError::new(
            StructureErrorKind::Antiunification,
            span.clone(),
        ));
    }
    let mut expfields_unified = vec![];
    for ((atom_template, exp_template), (_, exp)) in expfields_template.iter().zip(expfields) {
        let exp_template = antiunify_exp(frees, uenv, exp_template, exp)?;
        expfields_unified.push((atom_template.clone(), exp_template));
    }
    Ok(ExpKind::Str(expfields_unified))
}

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
            let var_unified = Var {
                id: id_unified.clone(),
                typ: typ.clone(),
                iters: iters.clone(),
            };
            if !vars_unified
                .iter()
                .any(|var: &Var| var.syntax_eq(&var_unified))
            {
                vars_unified.push(var_unified);
            }
        }
    }
    Ok(ExpKind::Iter(
        Box::new(exp_template),
        (*iter_template, vars_unified),
    ))
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

struct ExpTemplates {
    uenv: UEnv,
    exps: Vec<Exp>,
}

fn exps_span(exps: &[Exp], exps_template: &[Exp]) -> Span {
    let exps = if exps.is_empty() { exps_template } else { exps };
    Span::over(&exps.iter().map(|exp| exp.span.clone()).collect::<Vec<_>>())
}

fn antiunify_exps_group(
    mut frees: IdSet,
    exps_group: &[&[Exp]],
) -> Result<ExpTemplates, StructureError> {
    let Some((exps_head, exps_tail)) = exps_group.split_first() else {
        return Ok(ExpTemplates {
            uenv: UEnv::default(),
            exps: vec![],
        });
    };
    for exps in exps_tail {
        check_arity(exps_head.len(), exps.len(), &exps_span(exps, exps_head))?;
    }
    let mut uenv_acc = UEnv::default();
    let mut exps_template = vec![];
    for (exp_idx, exp_head) in exps_head.iter().enumerate() {
        let mut uenv = UEnv::default();
        let mut exp_template = exp_head.clone();
        for exps in exps_tail {
            exp_template = antiunify_exp(&mut frees, &mut uenv, &exp_template, &exps[exp_idx])?;
        }
        uenv_acc.extend(uenv)?;
        exps_template.push(exp_template);
    }
    Ok(ExpTemplates {
        uenv: uenv_acc,
        exps: exps_template,
    })
}

// Populating argument templates

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
        _ => Err(StructureError::new(
            StructureErrorKind::IncompatibleArguments,
            arg.span.clone(),
        )),
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
        prems.extend(populate_arg_template(uenv, arg_template, arg)?);
    }
    Ok(prems)
}

// Anti-unification of arguments

fn antiunify_arg(
    frees: &mut IdSet,
    uenv: &mut UEnv,
    arg_template: &Arg,
    arg: &Arg,
) -> Result<Arg, StructureError> {
    match (&arg_template.node, &arg.node) {
        (ArgKind::Exp(exp_template), ArgKind::Exp(exp)) => {
            antiunify_exp_arg(frees, uenv, arg_template, exp_template, exp)
        }
        (ArgKind::Def(id_template), ArgKind::Def(id)) if id_template.syntax_eq(id) => {
            Ok(arg_template.clone())
        }
        _ => Err(StructureError::new(
            StructureErrorKind::IncompatibleArguments,
            arg.span.clone(),
        )),
    }
}

fn antiunify_exp_arg(
    frees: &mut IdSet,
    uenv: &mut UEnv,
    arg_template: &Arg,
    exp_template: &Exp,
    exp: &Exp,
) -> Result<Arg, StructureError> {
    let exp_template = antiunify_exp(frees, uenv, exp_template, exp)?;
    Ok(crate::phrase! {node: ArgKind::Exp(Box::new(exp_template)), span: arg_template.span.clone()})
}

struct ArgTemplates {
    uenv: UEnv,
    args: Vec<Arg>,
}

fn antiunify_args_group(
    mut frees: IdSet,
    clauses: &[&Clause],
) -> Result<ArgTemplates, StructureError> {
    let Some((clause_head, clauses_tail)) = clauses.split_first() else {
        return Ok(ArgTemplates {
            uenv: UEnv::default(),
            args: vec![],
        });
    };
    let args_head = &clause_head.node.args;
    for clause in clauses_tail {
        check_arity(args_head.len(), clause.node.args.len(), &clause.span)?;
    }
    let mut uenv_acc = UEnv::default();
    let mut args_template = vec![];
    for (arg_idx, arg_head) in args_head.iter().enumerate() {
        let mut uenv = UEnv::default();
        let mut arg_template = arg_head.clone();
        for clause in clauses_tail {
            arg_template = antiunify_arg(
                &mut frees,
                &mut uenv,
                &arg_template,
                &clause.node.args[arg_idx],
            )?;
        }
        uenv_acc.extend(uenv)?;
        args_template.push(arg_template);
    }
    Ok(ArgTemplates {
        uenv: uenv_acc,
        args: args_template,
    })
}

// Anti-unification of rule matches

#[derive(Debug)]
pub(super) struct RuleMatchGroup {
    pub exps_template: Vec<Exp>,
    pub prems_group: Vec<Vec<Prem>>,
    pub prems_else: Option<Vec<Prem>>,
}

pub(super) fn antiunify_rule_match_group(
    frees: IdSet,
    exps_group: &[Vec<Exp>],
    exps_else: Option<&[Exp]>,
) -> Result<RuleMatchGroup, StructureError> {
    let exps_all = exps_group
        .iter()
        .map(Vec::as_slice)
        .chain(exps_else)
        .collect::<Vec<_>>();
    let ExpTemplates {
        uenv,
        exps: exps_template,
    } = antiunify_exps_group(frees, &exps_all)?;
    let prems_group = exps_group
        .iter()
        .map(|exps| {
            populate_exps_templates(
                &uenv,
                &exps_template,
                exps,
                &exps_span(exps, &exps_template),
            )
        })
        .collect::<Result<_, _>>()?;
    let prems_else = exps_else
        .map(|exps| {
            populate_exps_templates(
                &uenv,
                &exps_template,
                exps,
                &exps_span(exps, &exps_template),
            )
        })
        .transpose()?;
    Ok(RuleMatchGroup {
        exps_template,
        prems_group,
        prems_else,
    })
}

// Anti-unification of clauses

#[derive(Debug)]
pub(super) struct ClausePath {
    pub prems: Vec<Prem>,
    pub exp: Exp,
}

#[derive(Debug)]
pub(super) struct Clauses {
    pub args_template: Vec<Arg>,
    pub paths: Vec<ClausePath>,
    pub path_else: Option<ClausePath>,
}

pub(super) fn antiunify_clauses(
    clauses: Vec<Clause>,
    clause_else: Option<Clause>,
) -> Result<Clauses, StructureError> {
    let clauses_all = clauses.iter().chain(clause_else.iter()).collect::<Vec<_>>();
    let mut frees = IdSet::new();
    for clause in &clauses_all {
        clause.free_into(&mut frees);
    }
    let ArgTemplates {
        uenv,
        args: args_template,
    } = antiunify_args_group(frees, &clauses_all)?;
    let paths = clauses
        .into_iter()
        .map(|clause| populate_clause(&uenv, &args_template, clause))
        .collect::<Result<_, _>>()?;
    let path_else = clause_else
        .map(|clause| populate_clause(&uenv, &args_template, clause))
        .transpose()?;
    Ok(Clauses {
        args_template,
        paths,
        path_else,
    })
}

fn populate_clause(
    uenv: &UEnv,
    args_template: &[Arg],
    clause: Clause,
) -> Result<ClausePath, StructureError> {
    let clause_kind = clause.node;
    let ClauseKind { args, exp, prems } = clause_kind;
    let mut prems_template = populate_args_templates(uenv, args_template, &args, &clause.span)?;
    prems_template.extend(prems);
    Ok(ClausePath {
        prems: prems_template,
        exp,
    })
}
