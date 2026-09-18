//! AL invocation and ordered candidate selection

use super::super::backtrack::{choose_deterministic, choose_sequential};
use crate::runtime::envs::interp::al::ast_prepared as ast;
use crate::runtime::envs::interp::shared::frame::FrameLayout;
use std::rc::Rc;

use super::super::{
    AlInterp,
    context::{Context, Scope},
};
use super::{assign, expr, prem::eval_prems};
use crate::interp::shared::error::{CallErrorKind, GuardErrorKind, HostErrorKind, TraceErrorKind};
use crate::interp::shared::{
    backtrack::{Backtrack, err, ok, unmatch, unwrap, unwrap_from_result},
    cache::CallKey,
    error::{Error, ErrorKind},
};
use crate::lang::data::value::{ValueArena, ValueKind};
use crate::{
    lang::{data::value::Value, traits::print::Print},
    runner::{Extern, Interface, InterfaceError, RunnerContext},
    runtime::typdef::TypeDef,
};

// = Input and output checks

pub(in crate::interp::al) fn check_rel_inputs(
    arena: &ValueArena,
    ctx: &Context<'_>,
    id: &ast::Id,
    values: &[Value],
) -> Backtrack<()> {
    let rel = unwrap_from_result!(ctx.find_rel(id), &id.span);
    let (not_typ, inputs) = match &rel.def {
        ast::RelDef::Extern(rel) => (&rel.not_typ, &rel.input_hint),
        ast::RelDef::Defined(rel) => (&rel.not_typ, &rel.input_hint),
    };
    let typs = not_typ.node.args();
    unwrap_from_result!(crate::lang::hints::input::validate(inputs, typs.len()), &id.span);
    let typs = inputs
        .indices()
        .iter()
        .map(|index| typs[*index].clone())
        .collect::<Vec<_>>();
    check_values(
        arena,
        ctx,
        id,
        &typs,
        values,
        GuardErrorKind::RelationInputMismatch { relation: id.node.clone() },
    )
}

pub(in crate::interp::al) fn check_func_inputs(
    arena: &ValueArena,
    ctx: &Context<'_>,
    id: &ast::Id,
    targs: &[ast::Typ],
    values: &[Value],
) -> Backtrack<()> {
    let typ = unwrap_from_result!(ctx.find_func_typ(id), &id.span);
    unwrap!(Backtrack::check(
        typ.tparams.len() == targs.len(),
        id.span.clone(),
        ErrorKind::Call(CallErrorKind::TypeArgumentArityMismatch {
            expected: typ.tparams.len(),
            actual: targs.len()
        })
    ));
    let mut ctx_local = ctx.localize();
    for (tparam, targ) in typ.tparams.iter().zip(targs) {
        let def_typ =
            crate::phrase!(node: ast::DefTypKind::Plain(targ.clone()), span: targ.span.clone());
        unwrap_from_result!(
            ctx_local.add_typdef(tparam.clone(), TypeDef::Defined(vec![], Box::new(def_typ))),
            &tparam.span
        );
    }
    check_values(
        arena,
        &ctx_local,
        id,
        &typ.typs_params,
        values,
        GuardErrorKind::FunctionInputMismatch { func: id.node.clone() },
    )
}

fn check_values(
    arena: &ValueArena,
    ctx: &Context<'_>,
    id: &ast::Id,
    typs: &[ast::Typ],
    values: &[Value],
    error: GuardErrorKind,
) -> Backtrack<()> {
    let find_typdef_opt = |id: &ast::Id| ctx.find_typdef_opt(id);
    let find_func = |name: &str| {
        let id = crate::phrase!(node: name.to_owned(), span: id.span.clone());
        ctx.find_func_typ(&id).ok()
    };
    let matches = unwrap_from_result!(
        crate::runtime::ops::value::subs(arena, &find_typdef_opt, &find_func, typs, values),
        &id.span
    );
    Backtrack::check(matches, id.span.clone(), ErrorKind::Guard(error))
}

fn check_func_output(
    arena: &ValueArena,
    ctx: &Context<'_>,
    id: &ast::Id,
    tparams: &[ast::TParam],
    typ: &ast::Typ,
    targs: &[ast::Typ],
    value: &Value,
) -> Backtrack<()> {
    let theta =
        unwrap_from_result!(crate::runtime::ops::typ::Theta::from_lists(tparams, targs), &id.span);
    let typ = unwrap_from_result!(
        crate::runtime::ops::typ::subst_typ(&|id| theta.get(id), typ),
        &id.span
    );
    check_values(
        arena,
        ctx,
        id,
        &[typ],
        std::slice::from_ref(value),
        GuardErrorKind::FunctionOutputMismatch { func: id.node.clone() },
    )
}

// = Cache eligibility

pub(in crate::interp::al) fn cache_rel<Iface: Interface, Ext: Extern>(
    runner_ctx: &RunnerContext<'_, AlInterp, Iface, Ext>,
    ctx: &Context<'_>,
    id: &ast::Id,
) -> bool {
    runner_ctx.interp().config.cache
        && matches!(ctx.find_rel(id), Ok(rel) if matches!(&rel.def, ast::RelDef::Defined(_)))
}

pub(in crate::interp::al) fn cache_func<Iface: Interface, Ext: Extern>(
    runner_ctx: &RunnerContext<'_, AlInterp, Iface, Ext>,
    ctx: &Context<'_>,
    id: &ast::Id,
    values: &[Value],
) -> bool {
    runner_ctx.interp().config.cache
        && matches!(ctx.find_func(id), Ok((Scope::Global, func))
            if !matches!(&func.def, ast::MetaFuncDef::Extern(_)))
        && !values
            .iter()
            .any(|value| matches!(runner_ctx.arena().kind(value), ValueKind::Func(_)))
}

// = Relation invocation

pub fn invoke_rel<Iface: Interface, Ext: Extern>(
    runner_ctx: &mut RunnerContext<'_, AlInterp, Iface, Ext>,
    ctx: &Context<'_>,
    id: &ast::Id,
    values: &[Value],
) -> Backtrack<Vec<Value>> {
    let key =
        cache_rel(runner_ctx, ctx, id).then(|| CallKey::new(runner_ctx.arena(), &id.node, values));
    if let Some(values) = key
        .as_ref()
        .and_then(|key| runner_ctx.interp().cache.rels.get(key))
    {
        return ok!(values.clone());
    }
    runner_ctx.interp_mut().cache.begin();
    let result = stacker::maybe_grow(64 * 1024, 1024 * 1024, || {
        let rel = unwrap_from_result!(ctx.find_rel(id), &id.span);
        let layout = &rel.layout;
        match &rel.def {
            ast::RelDef::Extern(rel) => invoke_extern_rel(runner_ctx, ctx, id, rel, values),
            ast::RelDef::Defined(rel) => {
                invoke_defined_rel(runner_ctx, ctx, layout, id, rel, values)
            }
        }
    });
    let pure = runner_ctx.interp_mut().cache.end();
    if pure && let (Some(key), ok!(values)) = (key, &result) {
        runner_ctx
            .interp_mut()
            .cache
            .rels
            .insert(key, values.clone());
    }
    result.nest(id.span.clone(), || {
        ErrorKind::Trace(TraceErrorKind::Invocation { text: id.node.clone() })
    })
}

// - Extern relation

fn invoke_extern_rel<Iface: Interface, Ext: Extern>(
    runner_ctx: &mut RunnerContext<'_, AlInterp, Iface, Ext>,
    ctx: &Context<'_>,
    id: &ast::Id,
    rel: &ast::ExternRel,
    values: &[Value],
) -> Backtrack<Vec<Value>> {
    let result = runner_ctx.call_extern_rel(&id.node, values);
    runner_ctx
        .interp_mut()
        .cache
        .mark_effect(result.as_ref().map_or(true, |(_, effect)| *effect));
    let (values, _) = unwrap_from_result!(result, &id.span);
    if runner_ctx.interp().config.guard {
        let typs = rel.not_typ.node.args().into_iter().cloned().collect();
        let (_, typs) =
            unwrap_from_result!(crate::lang::hints::input::split(&rel.input_hint, typs), &id.span);
        unwrap!(check_values(
            runner_ctx.arena(),
            ctx,
            id,
            &typs,
            &values,
            GuardErrorKind::RelationOutputMismatch { relation: id.node.clone() }
        ));
    }
    ok!(values)
}

// - Defined relation

fn eval_rule_path<Iface: Interface, Ext: Extern>(
    runner_ctx: &mut RunnerContext<'_, AlInterp, Iface, Ext>,
    ctx: &Context<'_>,
    layout: &Rc<FrameLayout>,
    rule_match: &ast::RuleMatch,
    path: &ast::RulePath,
    values: &[Value],
) -> Backtrack<Vec<Value>> {
    unwrap!(Backtrack::check(
        rule_match.exps_input.len() == values.len(),
        path.id.span.clone(),
        ErrorKind::Call(CallErrorKind::RuleArityMismatch {
            expected: rule_match.exps_input.len(),
            actual: values.len()
        })
    ));
    let ctx = unwrap!(assign::assign_exps(
        runner_ctx.arena_mut(),
        ctx.localize_with_layout(layout),
        &rule_match.exps_input,
        values
    ));
    let ctx = unwrap!(eval_prems(runner_ctx, ctx, &rule_match.prems));
    let ctx = unwrap!(eval_prems(runner_ctx, ctx, &path.prems));
    expr::eval_exps(runner_ctx, &ctx, &path.exps_output)
}

fn invoke_defined_rel<Iface: Interface, Ext: Extern>(
    runner_ctx: &mut RunnerContext<'_, AlInterp, Iface, Ext>,
    ctx: &Context<'_>,
    layout: &Rc<FrameLayout>,
    id: &ast::Id,
    rel: &ast::DefinedRel,
    values: &[Value],
) -> Backtrack<Vec<Value>> {
    let det = runner_ctx.interp().config.det;
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
        eval_rule_path(runner_ctx, ctx, layout, &group.rule_match, path, values).nest(
            id.span.clone(),
            || {
                ErrorKind::Trace(TraceErrorKind::Evaluation {
                    text: format!("{}/{}/{}", id.node, group.id.node, path.id.node),
                })
            },
        )
    };
    let result = if det {
        choose_deterministic(paths, &mut evaluate, |(group_a, path_a), (group_b, path_b)| {
            Error::new(
                ErrorKind::Call(CallErrorKind::RelationNondeterminism {
                    relation: id.node.clone(),
                    group_a: group_a.id.node.clone(),
                    path_a: path_a.id.node.clone(),
                    group_b: group_b.id.node.clone(),
                    path_b: path_b.id.node.clone(),
                }),
                id.span.clone(),
            )
        })
    } else {
        choose_sequential(paths, &mut evaluate)
    };
    match result {
        ok!(values) => ok!(values),
        err!(errors) => err!(errors),
        unmatch!(errors) => match &rel.else_group {
            Some(group) => eval_rule_path(
                runner_ctx,
                ctx,
                layout,
                &group.node.rule_match,
                &group.node.rule_path,
                values,
            )
            .nest(id.span.clone(), || {
                ErrorKind::Trace(TraceErrorKind::Evaluation {
                    text: format!(
                        "{}/{}/{}",
                        id.node, group.node.id.node, group.node.rule_path.id.node
                    ),
                })
            }),
            None => unmatch!(errors),
        },
    }
}

// = Function invocation

pub fn invoke_func<Iface: Interface, Ext: Extern>(
    runner_ctx: &mut RunnerContext<'_, AlInterp, Iface, Ext>,
    ctx: &Context<'_>,
    id: &ast::Id,
    targs: &[ast::Typ],
    values: &[Value],
) -> Backtrack<Value> {
    let key = cache_func(runner_ctx, ctx, id, values)
        .then(|| CallKey::new(runner_ctx.arena(), &id.node, values));
    if let Some(value) = key
        .as_ref()
        .and_then(|key| runner_ctx.interp().cache.funcs.get(key))
    {
        return ok!(*value);
    }
    runner_ctx.interp_mut().cache.begin();
    let result = stacker::maybe_grow(64 * 1024, 1024 * 1024, || {
        let (_, func) = unwrap_from_result!(ctx.find_func(id), &id.span);
        let layout = &func.layout;
        match &func.def {
            ast::MetaFuncDef::Extern(func) => {
                invoke_extern_func(runner_ctx, ctx, id, func, targs, values)
            }
            ast::MetaFuncDef::Builtin(func) => {
                invoke_builtin_func(runner_ctx, ctx, id, func, targs, values)
            }
            ast::MetaFuncDef::Table(func) => {
                invoke_table_func(runner_ctx, ctx, layout, func, values)
            }
            ast::MetaFuncDef::Defined(func) => {
                invoke_defined_func(runner_ctx, ctx, layout, func, targs, values)
            }
        }
    });
    let pure = runner_ctx.interp_mut().cache.end();
    if pure && let (Some(key), ok!(value)) = (key, &result) {
        runner_ctx.interp_mut().cache.funcs.insert(key, *value);
    }
    result.nest(id.span.clone(), || ErrorKind::Trace(TraceErrorKind::function(id, targs)))
}

// - Extern function

fn invoke_extern_func<Iface: Interface, Ext: Extern>(
    runner_ctx: &mut RunnerContext<'_, AlInterp, Iface, Ext>,
    ctx: &Context<'_>,
    id: &ast::Id,
    extern_func: &ast::ExternFunc,
    targs: &[ast::Typ],
    values: &[Value],
) -> Backtrack<Value> {
    let result = runner_ctx.call_extern_func(&id.node, &[], values);
    runner_ctx
        .interp_mut()
        .cache
        .mark_effect(result.as_ref().map_or(true, |(_, effect)| *effect));
    let (value, _) = unwrap_from_result!(result, &id.span);
    if runner_ctx.interp().config.guard {
        unwrap!(check_func_output(
            runner_ctx.arena(),
            ctx,
            id,
            &extern_func.tparams,
            &extern_func.typ,
            targs,
            &value
        ));
    }
    ok!(value)
}

// - Builtin function

fn invoke_builtin_func<Iface: Interface, Ext: Extern>(
    runner_ctx: &mut RunnerContext<'_, AlInterp, Iface, Ext>,
    ctx: &Context<'_>,
    id: &ast::Id,
    builtin_func: &ast::BuiltinFunc,
    targs: &[ast::Typ],
    values: &[Value],
) -> Backtrack<Value> {
    let result = runner_ctx.call_builtin(id, targs, values);
    runner_ctx
        .interp_mut()
        .cache
        .mark_effect(result.as_ref().map_or(true, |(_, effect)| *effect));
    match result {
        Ok((value, _)) => {
            if runner_ctx.interp().config.guard {
                unwrap!(check_func_output(
                    runner_ctx.arena(),
                    ctx,
                    id,
                    &builtin_func.tparams,
                    &builtin_func.typ,
                    targs,
                    &value
                ));
            }
            ok!(value)
        }
        Err(error) => {
            let recoverable = matches!(
                error.kind.as_ref(),
                ErrorKind::Host(HostErrorKind::Interface(InterfaceError::Builtin(_)))
            );
            let error = error.at_if_missing(&id.span);
            if recoverable { unmatch!(vec![error]) } else { err!(vec![error]) }
        }
    }
}

// - Table function

fn eval_table_row<Iface: Interface, Ext: Extern>(
    runner_ctx: &mut RunnerContext<'_, AlInterp, Iface, Ext>,
    ctx: &Context<'_>,
    layout: &Rc<FrameLayout>,
    table_row: &ast::TableRow,
    values: &[Value],
) -> Backtrack<Value> {
    let result = (|| {
        unwrap!(Backtrack::check(
            table_row.node.args.len() == values.len(),
            table_row.span.clone(),
            ErrorKind::Call(CallErrorKind::TableRowArityMismatch {
                expected: table_row.node.args.len(),
                actual: values.len()
            })
        ));
        let ctx = unwrap!(assign::assign_args(
            runner_ctx.arena_mut(),
            ctx,
            ctx.localize_with_layout(layout),
            &table_row.node.args,
            values
        ));
        let ctx = unwrap!(eval_prems(runner_ctx, ctx, &table_row.node.prems));
        expr::eval_exp(runner_ctx, &ctx, &table_row.node.exp)
    })();
    result.nest(table_row.span.clone(), || {
        ErrorKind::Trace(TraceErrorKind::Evaluation { text: Print::to_string(table_row) })
    })
}

fn invoke_table_func<Iface: Interface, Ext: Extern>(
    runner_ctx: &mut RunnerContext<'_, AlInterp, Iface, Ext>,
    ctx: &Context<'_>,
    layout: &Rc<FrameLayout>,
    table_func: &ast::TableFunc,
    values: &[Value],
) -> Backtrack<Value> {
    choose_sequential(&table_func.table_rows, |table_row| {
        eval_table_row(runner_ctx, ctx, layout, table_row, values)
    })
}

// - Defined function

fn eval_clause<Iface: Interface, Ext: Extern>(
    runner_ctx: &mut RunnerContext<'_, AlInterp, Iface, Ext>,
    ctx_caller: &Context<'_>,
    ctx_callee: &Context<'_>,
    clause: &ast::Clause,
    values: &[Value],
) -> Backtrack<Value> {
    let result = (|| {
        unwrap!(Backtrack::check(
            clause.node.args.len() == values.len(),
            clause.span.clone(),
            ErrorKind::Call(CallErrorKind::ClauseArityMismatch {
                expected: clause.node.args.len(),
                actual: values.len()
            })
        ));
        let ctx = unwrap!(assign::assign_args(
            runner_ctx.arena_mut(),
            ctx_caller,
            ctx_callee.clone(),
            &clause.node.args,
            values
        ));
        let ctx = unwrap!(eval_prems(runner_ctx, ctx, &clause.node.prems));
        expr::eval_exp(runner_ctx, &ctx, &clause.node.exp)
    })();
    result.nest(clause.span.clone(), || {
        ErrorKind::Trace(TraceErrorKind::Evaluation { text: Print::to_string(clause) })
    })
}

fn invoke_defined_func<Iface: Interface, Ext: Extern>(
    runner_ctx: &mut RunnerContext<'_, AlInterp, Iface, Ext>,
    ctx: &Context<'_>,
    layout: &Rc<FrameLayout>,
    defined_func: &ast::DefinedFunc,
    targs: &[ast::Typ],
    values: &[Value],
) -> Backtrack<Value> {
    unwrap!(Backtrack::check(
        defined_func.tparams.len() == targs.len(),
        defined_func.id.span.clone(),
        ErrorKind::Call(CallErrorKind::TypeArgumentArityMismatch {
            expected: defined_func.tparams.len(),
            actual: targs.len()
        })
    ));
    let mut ctx_local = ctx.localize_with_layout(layout);
    for (tparam, targ) in defined_func.tparams.iter().zip(targs) {
        let def_typ =
            crate::phrase!(node: ast::DefTypKind::Plain(targ.clone()), span: targ.span.clone());
        unwrap_from_result!(
            ctx_local.add_typdef(tparam.clone(), TypeDef::Defined(vec![], Box::new(def_typ))),
            &tparam.span
        );
    }
    let det = runner_ctx.interp().config.det;
    let mut evaluate =
        |idx: &usize| eval_clause(runner_ctx, ctx, &ctx_local, &defined_func.clauses[*idx], values);
    let result = if det {
        choose_deterministic(0..defined_func.clauses.len(), &mut evaluate, |idx_a, idx_b| {
            Error::new(
                ErrorKind::Call(CallErrorKind::FunctionNondeterminism {
                    func: defined_func.id.node.clone(),
                    first: idx_a,
                    second: idx_b,
                }),
                defined_func.id.span.clone(),
            )
        })
    } else {
        choose_sequential(0..defined_func.clauses.len(), &mut evaluate)
    };
    match result {
        ok!(value) => ok!(value),
        err!(errors) => err!(errors),
        unmatch!(errors) => match &defined_func.else_clause {
            Some(clause) => eval_clause(runner_ctx, ctx, &ctx_local, clause, values),
            None => unmatch!(errors),
        },
    }
}
