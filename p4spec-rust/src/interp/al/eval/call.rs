//! AL invocation and ordered candidate selection

use super::super::{
    Al,
    backtrack::{Backtrack, back, choose_deterministic, choose_sequential},
    context::Context,
    error::ErrorKind,
};
use super::{assign, expr, prem::eval_prems};
use crate::interp::al::error::{CallErrorKind, HostErrorKind, TraceErrorKind};
use crate::{
    lang::{al::ast, data::value::Value, traits::print::Print},
    runner::{Extern, Interface, InterfaceError, RunnerContext},
    runtime::typdef::TypeDef,
};
use std::rc::Rc;

// = Relation invocation

pub fn invoke_rel<I: Interface, E: Extern>(
    runner: &mut RunnerContext<'_, Al, I, E>,
    ctx: &Context<'_>,
    id: &ast::Id,
    values: &[Rc<Value>],
) -> Backtrack<Vec<Rc<Value>>> {
    let result = stacker::maybe_grow(64 * 1024, 1024 * 1024, || {
        let rel = back!(Backtrack::from_result(ctx.find_rel(id), &id.span));
        match rel {
            ast::RelDef::Extern(_) => invoke_extern_rel(runner, id, values),
            ast::RelDef::Defined(rel) => invoke_defined_rel(runner, ctx, id, rel, values),
        }
    });
    result.nest(id.span.clone(), || {
        ErrorKind::Trace(TraceErrorKind::RelationInvocation {
            relation: id.node.clone(),
        })
    })
}

// - Extern relation

fn invoke_extern_rel<I: Interface, E: Extern>(
    runner: &mut RunnerContext<'_, Al, I, E>,
    id: &ast::Id,
    values: &[Rc<Value>],
) -> Backtrack<Vec<Rc<Value>>> {
    let (values, _) = back!(Backtrack::from_result(
        runner.call_extern_rel(&id.node, values),
        &id.span
    ));
    Backtrack::Ok(values)
}

// - Defined relation

fn eval_rule_path<I: Interface, E: Extern>(
    runner: &mut RunnerContext<'_, Al, I, E>,
    ctx: &Context<'_>,
    rule_match: &ast::RuleMatch,
    path: &ast::RulePath,
    values: &[Rc<Value>],
) -> Backtrack<Vec<Rc<Value>>> {
    back!(Backtrack::check(
        rule_match.exps_input.len() == values.len(),
        path.id.span.clone(),
        ErrorKind::Call(CallErrorKind::RuleArityMismatch {
            expected: rule_match.exps_input.len(),
            actual: values.len()
        })
    ));
    let ctx = back!(assign::assign_exps(
        &ctx.localize(),
        &rule_match.exps_input,
        values
    ));
    let ctx = back!(eval_prems(runner, &ctx, &rule_match.prems));
    let ctx = back!(eval_prems(runner, &ctx, &path.prems));
    expr::eval_exps(runner, &ctx, &path.exps_output)
}

fn invoke_defined_rel<I: Interface, E: Extern>(
    runner: &mut RunnerContext<'_, Al, I, E>,
    ctx: &Context<'_>,
    id: &ast::Id,
    rel: &ast::DefinedRel,
    values: &[Rc<Value>],
) -> Backtrack<Vec<Rc<Value>>> {
    let det = runner.config().det;
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
        eval_rule_path(runner, ctx, &group.rule_match, path, values).nest(id.span.clone(), || {
            ErrorKind::Trace(TraceErrorKind::RuleApplication {
                relation: id.node.clone(),
                group: group.id.node.clone(),
                path: path.id.node.clone(),
            })
        })
    };
    let result = if det {
        choose_deterministic(paths, &mut evaluate)
    } else {
        choose_sequential(paths, &mut evaluate).with_candidates()
    };
    match result {
        Backtrack::Ok(values) => Backtrack::Ok(values),
        Backtrack::Err(errors) => Backtrack::Err(errors),
        Backtrack::Unmatch(errors) => match &rel.else_group {
            Some(group) => eval_rule_path(
                runner,
                ctx,
                &group.node.rule_match,
                &group.node.rule_path,
                values,
            )
            .nest(id.span.clone(), || {
                ErrorKind::Trace(TraceErrorKind::RuleApplication {
                    relation: id.node.clone(),
                    group: group.node.id.node.clone(),
                    path: group.node.rule_path.id.node.clone(),
                })
            }),
            None => Backtrack::Unmatch(errors),
        },
        Backtrack::Nondet((group_a, path_a), (group_b, path_b)) => Backtrack::err(
            id.span.clone(),
            ErrorKind::Call(CallErrorKind::RelationNondeterminism {
                relation: id.node.clone(),
                group_a: group_a.id.node.clone(),
                path_a: path_a.id.node.clone(),
                group_b: group_b.id.node.clone(),
                path_b: path_b.id.node.clone(),
            }),
        ),
    }
}

// = Function invocation

pub fn invoke_func<I: Interface, E: Extern>(
    runner: &mut RunnerContext<'_, Al, I, E>,
    ctx: &Context<'_>,
    id: &ast::Id,
    targs: &[ast::Typ],
    values: &[Rc<Value>],
) -> Backtrack<Rc<Value>> {
    let result = stacker::maybe_grow(64 * 1024, 1024 * 1024, || {
        let (_, func) = back!(Backtrack::from_result(ctx.find_func(id), &id.span));
        match func {
            ast::MetaFuncDef::Extern(func) => invoke_extern_func(runner, func, values),
            ast::MetaFuncDef::Builtin(func) => invoke_builtin_func(runner, func, targs, values),
            ast::MetaFuncDef::Table(func) => invoke_table_func(runner, ctx, func, values),
            ast::MetaFuncDef::Defined(func) => {
                invoke_defined_func(runner, ctx, func, targs, values)
            }
        }
    });
    result.nest(id.span.clone(), || {
        ErrorKind::Trace(TraceErrorKind::FunctionInvocation {
            function: id.node.clone(),
            type_arguments: if targs.is_empty() {
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
            },
        })
    })
}

// - Extern function

fn invoke_extern_func<I: Interface, E: Extern>(
    runner: &mut RunnerContext<'_, Al, I, E>,
    extern_func: &ast::ExternFunc,
    values: &[Rc<Value>],
) -> Backtrack<Rc<Value>> {
    let (value, _) = back!(Backtrack::from_result(
        runner.call_extern_func(&extern_func.id.node, &[], values),
        &extern_func.id.span
    ));
    Backtrack::Ok(value)
}

// - Builtin function

fn invoke_builtin_func<I: Interface, E: Extern>(
    runner: &mut RunnerContext<'_, Al, I, E>,
    builtin_func: &ast::BuiltinFunc,
    targs: &[ast::Typ],
    values: &[Rc<Value>],
) -> Backtrack<Rc<Value>> {
    match runner.call_builtin(&builtin_func.id, targs, values) {
        Ok((value, _)) => Backtrack::Ok(value),
        Err(error) => {
            let recoverable = matches!(
                error.kind.as_ref(),
                ErrorKind::Host(HostErrorKind::Interface(InterfaceError::Builtin(_)))
            );
            let error = error.at_if_missing(&builtin_func.id.span);
            if recoverable {
                Backtrack::Unmatch(vec![error])
            } else {
                Backtrack::Err(vec![error])
            }
        }
    }
}

// - Table function

fn eval_table_row<I: Interface, E: Extern>(
    runner: &mut RunnerContext<'_, Al, I, E>,
    ctx: &Context<'_>,
    id: &ast::Id,
    table_row: &ast::TableRow,
    values: &[Rc<Value>],
) -> Backtrack<Rc<Value>> {
    let result = (|| {
        back!(Backtrack::check(
            table_row.node.args.len() == values.len(),
            table_row.span.clone(),
            ErrorKind::Call(CallErrorKind::TableRowArityMismatch {
                expected: table_row.node.args.len(),
                actual: values.len()
            })
        ));
        let ctx = back!(assign::assign_args(
            ctx,
            &ctx.localize(),
            &table_row.node.args,
            values
        ));
        let ctx = back!(eval_prems(runner, &ctx, &table_row.node.prems));
        expr::eval_exp(runner, &ctx, &table_row.node.exp)
    })();
    result.nest(id.span.clone(), || {
        ErrorKind::Trace(TraceErrorKind::TableRowApplication {
            function: id.node.clone(),
            arguments: Print::to_string(table_row.node.args.as_slice()),
        })
    })
}

fn invoke_table_func<I: Interface, E: Extern>(
    runner: &mut RunnerContext<'_, Al, I, E>,
    ctx: &Context<'_>,
    table_func: &ast::TableFunc,
    values: &[Rc<Value>],
) -> Backtrack<Rc<Value>> {
    choose_sequential(&table_func.table_rows, |table_row| {
        eval_table_row(runner, ctx, &table_func.id, table_row, values)
    })
}

// - Defined function

fn eval_clause<I: Interface, E: Extern>(
    runner: &mut RunnerContext<'_, Al, I, E>,
    ctx_caller: &Context<'_>,
    ctx_callee: &Context<'_>,
    defined_func: &ast::DefinedFunc,
    clause: &ast::Clause,
    values: &[Rc<Value>],
) -> Backtrack<Rc<Value>> {
    let result = (|| {
        back!(Backtrack::check(
            clause.node.args.len() == values.len(),
            clause.span.clone(),
            ErrorKind::Call(CallErrorKind::ClauseArityMismatch {
                expected: clause.node.args.len(),
                actual: values.len()
            })
        ));
        let ctx = back!(assign::assign_args(
            ctx_caller,
            ctx_callee,
            &clause.node.args,
            values
        ));
        let ctx = back!(eval_prems(runner, &ctx, &clause.node.premises));
        expr::eval_exp(runner, &ctx, &clause.node.expression)
    })();
    result.nest(defined_func.id.span.clone(), || {
        ErrorKind::Trace(TraceErrorKind::ClauseApplication {
            function: defined_func.id.node.clone(),
            arguments: Print::to_string(clause.node.args.as_slice()),
        })
    })
}

fn invoke_defined_func<I: Interface, E: Extern>(
    runner: &mut RunnerContext<'_, Al, I, E>,
    ctx: &Context<'_>,
    defined_func: &ast::DefinedFunc,
    targs: &[ast::Typ],
    values: &[Rc<Value>],
) -> Backtrack<Rc<Value>> {
    back!(Backtrack::check(
        defined_func.tparams.len() == targs.len(),
        defined_func.id.span.clone(),
        ErrorKind::Call(CallErrorKind::TypeArgumentArityMismatch {
            expected: defined_func.tparams.len(),
            actual: targs.len()
        })
    ));
    let mut ctx_local = ctx.localize();
    for (tparam, targ) in defined_func.tparams.iter().zip(targs) {
        let def_typ =
            crate::phrase!(node: ast::DefTypKind::Plain(targ.clone()), span: targ.span.clone());
        back!(Backtrack::from_result(
            ctx_local.add_typdef(tparam.clone(), TypeDef::Defined(vec![], Box::new(def_typ))),
            &tparam.span
        ));
    }
    let det = runner.config().det;
    let mut evaluate = |idx: &usize| {
        eval_clause(
            runner,
            ctx,
            &ctx_local,
            defined_func,
            &defined_func.clauses[*idx],
            values,
        )
    };
    let result = if det {
        choose_deterministic(0..defined_func.clauses.len(), &mut evaluate)
    } else {
        choose_sequential(0..defined_func.clauses.len(), &mut evaluate).with_candidates()
    };
    match result {
        Backtrack::Ok(value) => Backtrack::Ok(value),
        Backtrack::Err(errors) => Backtrack::Err(errors),
        Backtrack::Unmatch(errors) => match &defined_func.else_clause {
            Some(clause) => eval_clause(runner, ctx, &ctx_local, defined_func, clause, values),
            None => Backtrack::Unmatch(errors),
        },
        Backtrack::Nondet(idx_a, idx_b) => Backtrack::err(
            defined_func.id.span.clone(),
            ErrorKind::Call(CallErrorKind::FunctionNondeterminism {
                function: defined_func.id.node.clone(),
                first: idx_a,
                second: idx_b,
            }),
        ),
    }
}
