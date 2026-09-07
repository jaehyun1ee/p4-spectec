//! AL premises and invocation over the composed runner
//!
//! Each clause and rule path starts with fresh local bindings. The choice
//! primitives retain source order and distinguish a mismatch from a fatal
//! failure. Internal calls reuse the runner without crossing its public entry
//! boundary; observer events surround every function and relation invocation.

use super::{
    Al, assignment,
    backtrack::{Backtrack, choose_sequential},
    context::Context,
    error::ErrorKind,
    expression,
    nondet::{BacktrackDet, choose_deterministic},
};
use crate::interp::common::Event;
use crate::{
    lang::{
        al::ast,
        common::{Variable, source::Span},
        data::{
            typ,
            value::{Value, get, make},
        },
        hints::input,
        traits::print::Print,
    },
    runner::{Extern, Interface, InterfaceError, RunnerContext},
    runtime::typdef::TypeDef,
};
use std::{fmt::Display, rc::Rc};

macro_rules! back {
    ($result:expr) => {
        match $result {
            $crate::interp::al::backtrack::Backtrack::Ok(value) => value,
            $crate::interp::al::backtrack::Backtrack::Err(traces) => {
                return $crate::interp::al::backtrack::Backtrack::Err(traces)
            }
            $crate::interp::al::backtrack::Backtrack::Unmatch(traces) => {
                return $crate::interp::al::backtrack::Backtrack::Unmatch(traces)
            }
        }
    };
}
pub(super) use back;

pub(super) fn located<T>(result: Result<T, impl Display>, span: &Span) -> Backtrack<T> {
    match result {
        Ok(value) => Backtrack::Ok(value),
        Err(error) => Backtrack::err(span.clone(), error.to_string()),
    }
}

pub fn eval_prems<I: Interface, E: Extern>(
    runner: &mut RunnerContext<'_, Al, I, E>,
    ctx: &Context,
    prems: &[ast::Prem],
) -> Backtrack<Context> {
    let mut ctx = ctx.clone();
    for prem in prems {
        ctx = back!(eval_prem(runner, &ctx, prem));
    }
    Backtrack::Ok(ctx)
}

pub fn eval_prem<I: Interface, E: Extern>(
    runner: &mut RunnerContext<'_, Al, I, E>,
    ctx: &Context,
    prem: &ast::Prem,
) -> Backtrack<Context> {
    match &prem.node {
        ast::PremKind::Rule(prem) => {
            let exps = prem.not_exp.args().into_iter().cloned().collect();
            let (exps_input, exps_output) =
                back!(located(input::split(&prem.input_hint, exps), &prem.id.span));
            let values = back!(expression::eval_exps(runner, ctx, &exps_input));
            let values = back!(invoke_rel(runner, ctx, &prem.id, &values));
            assignment::assign_exps(ctx, &exps_output, &values)
        }
        ast::PremKind::If(prem) => {
            let value = back!(expression::eval_exp(runner, ctx, &prem.exp));
            if back!(located(get::bool(&value), &prem.exp.span)) {
                Backtrack::Ok(ctx.clone())
            } else {
                Backtrack::unmatch(
                    prem.exp.span.clone(),
                    format!("condition {} was not met", Print::to_string(&prem.exp)),
                )
            }
        }
        ast::PremKind::IfHold(prem) => {
            let exps: Vec<_> = prem.not_exp.args().into_iter().cloned().collect();
            let values = back!(expression::eval_exps(runner, ctx, &exps));
            match invoke_rel(runner, ctx, &prem.id, &values) {
                Backtrack::Ok(_) => Backtrack::Ok(ctx.clone()),
                Backtrack::Err(traces) => Backtrack::Err(traces),
                Backtrack::Unmatch(traces) => Backtrack::Unmatch(traces)
                    .nest(prem.id.span.clone(), || {
                        format!("condition hold {} was not met", prem.id.node)
                    }),
            }
        }
        ast::PremKind::IfNotHold(prem) => {
            let exps: Vec<_> = prem.not_exp.args().into_iter().cloned().collect();
            let values = back!(expression::eval_exps(runner, ctx, &exps));
            match invoke_rel(runner, ctx, &prem.id, &values) {
                Backtrack::Ok(_) => Backtrack::unmatch(
                    prem.id.span.clone(),
                    format!("condition not-hold {} was not met", prem.id.node),
                ),
                Backtrack::Err(traces) => Backtrack::Err(traces),
                Backtrack::Unmatch(_) => Backtrack::Ok(ctx.clone()),
            }
        }
        ast::PremKind::Let(prem) => {
            let value = back!(expression::eval_exp(runner, ctx, &prem.exp_r));
            assignment::assign_exp(ctx, &prem.exp_l, value)
        }
        ast::PremKind::Iter(prem) => {
            let iter = &prem.prem_iter;
            let ctxs = match iter.iter {
                ast::Iter::Opt => back!(located(ctx.sub_opt(&iter.vars_bound), &prem.prem.span))
                    .into_iter()
                    .collect(),
                ast::Iter::List => back!(located(ctx.sub_list(&iter.vars_bound), &prem.prem.span)),
            };
            let mut batches = vec![Vec::new(); iter.vars_bind.len()];
            for ctx_sub in ctxs {
                let ctx_sub = back!(eval_prem(runner, &ctx_sub, &prem.prem));
                for (var, values) in iter.vars_bind.iter().zip(&mut batches) {
                    let variable = Variable::new(var.id.clone(), var.iters.clone());
                    values.push(Rc::clone(back!(located(
                        ctx_sub.find_value(&variable),
                        &var.id.span
                    ))));
                }
            }
            let mut ctx = ctx.clone();
            for (var, values) in iter.vars_bind.iter().zip(batches) {
                let mut iters = var.iters.clone();
                iters.push(iter.iter);
                let typ = typ::make::iterate(var.typ.clone(), &iters);
                let value = match iter.iter {
                    ast::Iter::Opt => make::opt(&typ, values.into_iter().next(), Span::default()),
                    ast::Iter::List => make::list(&typ, values, Span::default()),
                };
                ctx.add_value(Variable::new(var.id.clone(), iters), value);
            }
            Backtrack::Ok(ctx)
        }
        ast::PremKind::Debug(prem) => {
            let value = back!(expression::eval_exp(runner, ctx, &prem.exp));
            let expression = Print::to_string(&prem.exp);
            println!("{}: {}", prem.exp.span, expression);
            let region = value.span.to_string();
            if region.is_empty() {
                println!("{}", Print::to_string(value.as_ref()));
            } else {
                println!("{region}: {}", Print::to_string(value.as_ref()));
            }
            runner.interp_state().emit(Event::Debug {
                span: prem.exp.span.clone(),
                expression,
                value,
            });
            Backtrack::Ok(ctx.clone())
        }
    }
}

pub fn invoke_rel<I: Interface, E: Extern>(
    runner: &mut RunnerContext<'_, Al, I, E>,
    ctx: &Context,
    id: &ast::Id,
    values: &[Rc<Value>],
) -> Backtrack<Vec<Rc<Value>>> {
    runner.interp_state().emit(Event::RelEnter {
        id: id.clone(),
        values: values.to_vec(),
    });
    let result = stacker::maybe_grow(64 * 1024, 1024 * 1024, || {
        let rel = back!(located(ctx.find_rel(runner.spec(), id), &id.span));
        match rel {
            ast::RelDef::Extern(_) => {
                let (values, _) =
                    back!(located(runner.call_extern_rel(&id.node, values), &id.span));
                Backtrack::Ok(values)
            }
            ast::RelDef::Defined(rel) => invoke_defined_rel(runner, ctx, id, rel, values),
        }
    });
    runner
        .interp_state()
        .emit(Event::RelExit { id: id.clone() });
    result.nest(id.span.clone(), || {
        format!("invocation of relation {} failed", id.node)
    })
}

fn invoke_rule_path<I: Interface, E: Extern>(
    runner: &mut RunnerContext<'_, Al, I, E>,
    ctx: &Context,
    rule_match: &ast::RuleMatch,
    path: &ast::RulePath,
    values: &[Rc<Value>],
) -> Backtrack<Vec<Rc<Value>>> {
    back!(Backtrack::check(
        rule_match.exps_input.len() == values.len(),
        path.id.span.clone(),
        "arity mismatch in rule"
    ));
    let ctx = back!(assignment::assign_exps(
        &ctx.localize(),
        &rule_match.exps_input,
        values
    ));
    let ctx = back!(eval_prems(runner, &ctx, &rule_match.prems));
    let ctx = back!(eval_prems(runner, &ctx, &path.prems));
    expression::eval_exps(runner, &ctx, &path.exps_output)
}

fn invoke_defined_rel<I: Interface, E: Extern>(
    runner: &mut RunnerContext<'_, Al, I, E>,
    ctx: &Context,
    id: &ast::Id,
    rel: &ast::DefinedRel,
    values: &[Rc<Value>],
) -> Backtrack<Vec<Rc<Value>>> {
    let det = runner.interp_state().det;
    let paths: Vec<_> = rel
        .rule_groups
        .iter()
        .flat_map(|group| {
            group
                .node
                .rule_paths
                .iter()
                .map(move |path| (&group.node, path))
        })
        .collect();
    let mut evaluate = |&(group, path): &(&ast::RuleGroupKind, &ast::RulePath)| {
        invoke_rule_path(runner, ctx, &group.rule_match, path, values).nest(id.span.clone(), || {
            format!(
                "application of rule {}/{}/{} failed",
                id.node, group.id.node, path.id.node
            )
        })
    };
    let result = if det {
        choose_deterministic(paths, &mut evaluate)
    } else {
        choose_sequential(paths, &mut evaluate).into()
    };
    match result {
        BacktrackDet::Ok(values) => Backtrack::Ok(values),
        BacktrackDet::Err(traces) => Backtrack::Err(traces),
        BacktrackDet::Unmatch(traces) => match &rel.else_group {
            Some(group) => invoke_rule_path(
                runner,
                ctx,
                &group.node.rule_match,
                &group.node.rule_path,
                values,
            )
            .nest(id.span.clone(), || {
                format!(
                    "application of rule {}/{}/{} failed",
                    id.node, group.node.id.node, group.node.rule_path.id.node
                )
            }),
            None => Backtrack::Unmatch(traces),
        },
        BacktrackDet::Nondet((group_a, path_a), (group_b, path_b)) => Backtrack::err(
            id.span.clone(),
            format!(
                "non-deterministic application of relation {}: {}/{}, {}/{}",
                id.node, group_a.id.node, path_a.id.node, group_b.id.node, path_b.id.node
            ),
        ),
    }
}

pub fn invoke_func<I: Interface, E: Extern>(
    runner: &mut RunnerContext<'_, Al, I, E>,
    ctx: &Context,
    id: &ast::Id,
    targs: &[ast::Typ],
    values: &[Rc<Value>],
) -> Backtrack<Rc<Value>> {
    runner.interp_state().emit(Event::FuncEnter {
        id: id.clone(),
        values: values.to_vec(),
    });
    let result = stacker::maybe_grow(64 * 1024, 1024 * 1024, || {
        let (_, func) = back!(located(ctx.find_func(runner.spec(), id), &id.span));
        match func {
            ast::MetaFuncDef::Extern(_) => {
                let (value, _) = back!(located(
                    runner.call_extern_func(&id.node, &[], values),
                    &id.span
                ));
                Backtrack::Ok(value)
            }
            ast::MetaFuncDef::Builtin(_) => match runner.call_builtin(id, targs, values) {
                Ok((value, _)) => Backtrack::Ok(value),
                Err(error) => match error.kind {
                    ErrorKind::Interface(InterfaceError::Builtin(error)) => {
                        Backtrack::unmatch(id.span.clone(), error.to_string())
                    }
                    kind => Backtrack::err(id.span.clone(), kind.to_string()),
                },
            },
            ast::MetaFuncDef::Table(func) => choose_sequential(&func.table_rows, |row| {
                let result = (|| {
                    back!(Backtrack::check(
                        row.node.args.len() == values.len(),
                        row.span.clone(),
                        "arity mismatch while matching table row"
                    ));
                    let ctx = back!(assignment::assign_args(
                        runner.spec(),
                        ctx,
                        &ctx.localize(),
                        &row.node.args,
                        values
                    ));
                    let ctx = back!(eval_prems(runner, &ctx, &row.node.prems));
                    expression::eval_exp(runner, &ctx, &row.node.exp)
                })();
                result.nest(id.span.clone(), || {
                    format!(
                        "application of table row {}{} failed",
                        id.node,
                        Print::to_string(row.node.args.as_slice())
                    )
                })
            }),
            ast::MetaFuncDef::Defined(func) => {
                invoke_defined_func(runner, ctx, id, func, targs, values)
            }
        }
    });
    runner
        .interp_state()
        .emit(Event::FuncExit { id: id.clone() });
    result.nest(id.span.clone(), || {
        format!(
            "invocation of function ${}{} failed",
            id.node,
            if targs.is_empty() {
                String::new()
            } else {
                format!(
                    "<{}>",
                    targs
                        .iter()
                        .map(Print::to_string)
                        .collect::<Vec<_>>()
                        .join(", ")
                )
            }
        )
    })
}

fn invoke_clause<I: Interface, E: Extern>(
    runner: &mut RunnerContext<'_, Al, I, E>,
    ctx: &Context,
    id: &ast::Id,
    func: &ast::DefinedFunc,
    clause: &ast::Clause,
    targs: &[ast::Typ],
    values: &[Rc<Value>],
) -> Backtrack<Rc<Value>> {
    let result = (|| {
        back!(Backtrack::check(
            func.tparams.len() == targs.len(),
            id.span.clone(),
            "arity mismatch in type arguments"
        ));
        let mut ctx_local = ctx.localize();
        for (tparam, targ) in func.tparams.iter().zip(targs) {
            let def_typ =
                crate::phrase!(node: ast::DefTypKind::Plain(targ.clone()), span: targ.span.clone());
            back!(located(
                ctx_local.add_typdef(
                    runner.spec(),
                    tparam.clone(),
                    TypeDef::Defined(vec![], Box::new(def_typ))
                ),
                &tparam.span
            ));
        }
        back!(Backtrack::check(
            clause.node.args.len() == values.len(),
            clause.span.clone(),
            "arity mismatch while matching clause"
        ));
        let ctx = back!(assignment::assign_args(
            runner.spec(),
            ctx,
            &ctx_local,
            &clause.node.args,
            values
        ));
        let ctx = back!(eval_prems(runner, &ctx, &clause.node.premises));
        expression::eval_exp(runner, &ctx, &clause.node.expression)
    })();
    result.nest(id.span.clone(), || {
        format!(
            "application of clause {}{} failed",
            id.node,
            Print::to_string(clause.node.args.as_slice())
        )
    })
}

fn invoke_defined_func<I: Interface, E: Extern>(
    runner: &mut RunnerContext<'_, Al, I, E>,
    ctx: &Context,
    id: &ast::Id,
    func: &ast::DefinedFunc,
    targs: &[ast::Typ],
    values: &[Rc<Value>],
) -> Backtrack<Rc<Value>> {
    let det = runner.interp_state().det;
    let mut evaluate =
        |index: &usize| invoke_clause(runner, ctx, id, func, &func.clauses[*index], targs, values);
    let result = if det {
        choose_deterministic(0..func.clauses.len(), &mut evaluate)
    } else {
        choose_sequential(0..func.clauses.len(), &mut evaluate).into()
    };
    match result {
        BacktrackDet::Ok(value) => Backtrack::Ok(value),
        BacktrackDet::Err(traces) => Backtrack::Err(traces),
        BacktrackDet::Unmatch(traces) => match &func.else_clause {
            Some(clause) => invoke_clause(runner, ctx, id, func, clause, targs, values),
            None => Backtrack::Unmatch(traces),
        },
        BacktrackDet::Nondet(a, b) => Backtrack::err(
            id.span.clone(),
            format!(
                "non-deterministic application of function {}: {a}, {b}",
                id.node
            ),
        ),
    }
}
