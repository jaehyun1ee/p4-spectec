//! SL invocation, memoization and tail-call dispatch

use super::super::{
    SlInterp,
    context::{Context, Scope},
};
use super::{
    assign,
    instr::{self, Flow},
};
use crate::{
    interp::shared::{
        backtrack::{Backtrack, backtrack, backtrack_from_result},
        cache::CallKey,
        error::{
            AssignErrorKind, CallErrorKind, ErrorKind, GuardErrorKind, HostErrorKind,
            TraceErrorKind,
        },
    },
    lang::{
        data::value::{Value, ValueArena, ValueKind},
        sl::ast,
        traits::print::Print,
    },
    runner::{Extern, Interface, InterfaceError, RunnerContext},
    runtime::typdef::TypeDef,
};
use std::{borrow::Cow, rc::Rc};
// = Input and output checks

pub(crate) fn check_rel_inputs(
    arena: &ValueArena,
    ctx: &Context<'_>,
    id: &ast::Id,
    values: &[Value],
) -> Backtrack<()> {
    let rel = backtrack_from_result!(ctx.find_rel(id), &id.span);
    let (not_typ, inputs) = match rel {
        ast::RelDef::Extern(rel) => (&rel.rel_signature.not_typ, &rel.rel_signature.input_hint),
        ast::RelDef::Defined(rel) => (&rel.rel_signature.not_typ, &rel.rel_signature.input_hint),
    };
    let typs = not_typ.node.args();
    backtrack_from_result!(crate::lang::hints::input::validate(inputs, typs.len()), &id.span);
    let typs = inputs
        .indices()
        .iter()
        .map(|index| typs[*index as usize].clone())
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

pub(crate) fn check_func_inputs(
    arena: &ValueArena,
    ctx: &Context<'_>,
    id: &ast::Id,
    targs: &[ast::Typ],
    values: &[Value],
) -> Backtrack<()> {
    let typ = backtrack_from_result!(ctx.find_func_typ(id), &id.span);
    backtrack!(Backtrack::check(
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
        backtrack_from_result!(
            ctx_local.bind_tparam(tparam.clone(), TypeDef::Defined(vec![], Box::new(def_typ))),
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
    let tdenv = ctx.tdenv();
    let find_func = |name: &str| {
        let id = crate::phrase!(node: name.to_owned(), span: id.span.clone());
        ctx.find_func_typ(&id).ok()
    };
    let matches = backtrack_from_result!(
        crate::runtime::ops::value::subs(arena, &tdenv, &find_func, typs, values),
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
    let theta = backtrack_from_result!(
        crate::runtime::ops::typ::Theta::from_lists(tparams, targs),
        &id.span
    );
    let typ = backtrack_from_result!(crate::runtime::ops::typ::subst_typ(&theta, typ), &id.span);
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

pub(crate) fn cache_rel<Iface: Interface, Exn: Extern>(
    runner: &RunnerContext<'_, SlInterp, Iface, Exn>,
    ctx: &Context<'_>,
    id: &ast::Id,
) -> bool {
    runner.interp().config.cache && matches!(ctx.find_rel(id), Ok(ast::RelDef::Defined(_)))
}

pub(crate) fn cache_func<Iface: Interface, Exn: Extern>(
    runner: &RunnerContext<'_, SlInterp, Iface, Exn>,
    ctx: &Context<'_>,
    id: &ast::Id,
    values: &[Value],
) -> bool {
    runner.interp().config.cache
        && matches!(ctx.find_func(id), Ok((Scope::Global, func))
            if !matches!(func.as_ref(), ast::MetaFuncDef::Extern(_)))
        && !values
            .iter()
            .any(|value| matches!(runner.arena().kind(value), ValueKind::Func(_)))
}

fn invoke_extern_rel<Iface: Interface, Exn: Extern>(
    runner: &mut RunnerContext<'_, SlInterp, Iface, Exn>,
    ctx: &Context<'_>,
    id: &ast::Id,
    rel: &ast::ExternRel,
    values: &[Value],
) -> Backtrack<Vec<Value>> {
    let result = runner.call_extern_rel(&id.node, values);
    runner
        .interp_mut()
        .cache
        .mark_effect(result.as_ref().map_or(true, |(_, effect)| *effect));
    let (values, _) = backtrack_from_result!(result, &id.span);
    if runner.interp().config.guard {
        let typs = rel
            .rel_signature
            .not_typ
            .node
            .args()
            .into_iter()
            .cloned()
            .collect();
        let (_, typs) = backtrack_from_result!(
            crate::lang::hints::input::split(&rel.rel_signature.input_hint, typs),
            &id.span
        );
        backtrack!(
            check_values(
                runner.arena(),
                ctx,
                id,
                &typs,
                &values,
                GuardErrorKind::RelationOutputMismatch { relation: id.node.clone() }
            )
            .guard()
        );
    }
    Backtrack::Ok(values)
}

fn invoke_extern_func<Iface: Interface, Exn: Extern>(
    runner: &mut RunnerContext<'_, SlInterp, Iface, Exn>,
    ctx: &Context<'_>,
    id: &ast::Id,
    extern_func: &ast::ExternFunc,
    targs: &[ast::Typ],
    values: &[Value],
) -> Backtrack<Value> {
    let result = runner.call_extern_func(&id.node, &[], values);
    runner
        .interp_mut()
        .cache
        .mark_effect(result.as_ref().map_or(true, |(_, effect)| *effect));
    let (value, _) = backtrack_from_result!(result, &id.span);
    if runner.interp().config.guard {
        backtrack!(
            check_func_output(
                runner.arena(),
                ctx,
                id,
                &extern_func.tparams,
                &extern_func.typ,
                targs,
                &value
            )
            .guard()
        );
    }
    Backtrack::Ok(value)
}

// - Builtin function

fn invoke_builtin_func<Iface: Interface, Exn: Extern>(
    runner: &mut RunnerContext<'_, SlInterp, Iface, Exn>,
    ctx: &Context<'_>,
    id: &ast::Id,
    builtin_func: &ast::BuiltinFunc,
    targs: &[ast::Typ],
    values: &[Value],
) -> Backtrack<Value> {
    let result = runner.call_builtin(id, targs, values);
    runner
        .interp_mut()
        .cache
        .mark_effect(result.as_ref().map_or(true, |(_, effect)| *effect));
    match result {
        Ok((value, _)) => {
            if runner.interp().config.guard {
                backtrack!(
                    check_func_output(
                        runner.arena(),
                        ctx,
                        id,
                        &builtin_func.tparams,
                        &builtin_func.typ,
                        targs,
                        &value
                    )
                    .guard()
                );
            }
            Backtrack::Ok(value)
        }
        Err(error) => {
            let recoverable = matches!(
                error.kind.as_ref(),
                ErrorKind::Host(HostErrorKind::Interface(InterfaceError::Builtin(_)))
            );
            let error = error.at_if_missing(&id.span);
            if recoverable { Backtrack::Unmatch(vec![error]) } else { Backtrack::Err(vec![error]) }
        }
    }
}

fn invoke_rel_mode<Iface: Interface, Exn: Extern>(
    runner: &mut RunnerContext<'_, SlInterp, Iface, Exn>,
    ctx: &Context<'_>,
    id: &ast::Id,
    values: &[Value],
    internal: bool,
) -> Backtrack<Vec<Value>> {
    let mut id = Cow::Borrowed(id);
    let mut values = Cow::Borrowed(values);
    let mut pending = Vec::new();
    loop {
        let key =
            cache_rel(runner, ctx, &id).then(|| CallKey::new(runner.arena(), &id.node, &values));
        if let Some(values) = key
            .as_ref()
            .and_then(|key| runner.interp().cache.rels.get(key))
        {
            return Backtrack::Ok(values.clone());
        }
        if !internal && runner.interp().config.guard && key.is_none() {
            backtrack!(check_rel_inputs(runner.arena(), ctx, &id, &values).guard());
        }
        runner.interp_mut().cache.begin();
        let result = stacker::maybe_grow(64 * 1024, 1024 * 1024, || {
            let rel = backtrack_from_result!(ctx.find_rel(&id), &id.span);
            match rel {
                ast::RelDef::Extern(rel) => {
                    let values = backtrack!(invoke_extern_rel(runner, ctx, &id, rel, &values));
                    Backtrack::Ok(Flow::Result(values))
                }
                ast::RelDef::Defined(rel) => {
                    let ctx = backtrack!(assign::assign_exps(
                        runner.arena_mut(),
                        ctx.localize(),
                        &rel.exps_input,
                        &values
                    ));
                    let flow = backtrack!(instr::eval_body(
                        runner,
                        ctx,
                        &rel.block,
                        rel.block_else.as_deref()
                    ));
                    match flow {
                        Flow::Result(_) | Flow::TailRel(..) => Backtrack::Ok(flow),
                        Flow::Cont(errors) => Backtrack::Unmatch(errors),
                        Flow::Return(_) => invalid_flow(&id, "relation cannot return a value"),
                        Flow::TailFunc(..) => {
                            invalid_flow(&id, "unexpected function tailcall in relation body")
                        }
                    }
                }
            }
        });
        let pure = runner.interp_mut().cache.end();
        let result = result.nest(id.span.clone(), || {
            ErrorKind::Trace(TraceErrorKind::RelationInvocation { rel: id.node.clone() })
        });
        let flow = match result {
            Backtrack::Ok(flow) => flow,
            Backtrack::Err(errors) => return nest_pending(Backtrack::Err(errors), pending),
            Backtrack::Unmatch(errors) => return nest_pending(Backtrack::Unmatch(errors), pending),
        };
        match flow {
            Flow::Result(values) => {
                if pure && let Some(key) = key {
                    runner.interp_mut().cache.rels.insert(key, values.clone());
                }
                return Backtrack::Ok(values);
            }
            Flow::TailRel(id_tail, values_tail) => {
                let id_pending = id.into_owned();
                pending.push((
                    id_pending.span,
                    TraceErrorKind::RelationInvocation { rel: id_pending.node },
                ));
                id = Cow::Owned(id_tail);
                values = Cow::Owned(values_tail);
            }
            _ => unreachable!("relation dispatch validates its flow"),
        }
    }
}

fn invoke_func_mode<Iface: Interface, Exn: Extern>(
    runner: &mut RunnerContext<'_, SlInterp, Iface, Exn>,
    ctx: &Context<'_>,
    id: &ast::Id,
    targs: &[ast::Typ],
    values: &[Value],
    internal: bool,
) -> Backtrack<Value> {
    let mut id = Cow::Borrowed(id);
    let mut targs = Cow::Borrowed(targs);
    let mut values = Cow::Borrowed(values);
    let mut pending = Vec::new();
    loop {
        let key = cache_func(runner, ctx, &id, &values)
            .then(|| CallKey::new(runner.arena(), &id.node, &values));
        if let Some(value) = key
            .as_ref()
            .and_then(|key| runner.interp().cache.funcs.get(key))
        {
            return Backtrack::Ok(*value);
        }
        if !internal && runner.interp().config.guard && key.is_none() {
            backtrack!(check_func_inputs(runner.arena(), ctx, &id, &targs, &values).guard());
        }
        runner.interp_mut().cache.begin();
        let result = stacker::maybe_grow(64 * 1024, 1024 * 1024, || {
            let (_, func) = backtrack_from_result!(ctx.find_func(&id), &id.span);
            match func.as_ref() {
                ast::MetaFuncDef::Extern(func) => Backtrack::Ok(Flow::Return(backtrack!(
                    invoke_extern_func(runner, ctx, &id, func, &targs, &values)
                ))),
                ast::MetaFuncDef::Builtin(func) => Backtrack::Ok(Flow::Return(backtrack!(
                    invoke_builtin_func(runner, ctx, &id, func, &targs, &values)
                ))),
                ast::MetaFuncDef::Table(func) => {
                    let ctx_local = backtrack!(assign_params(
                        runner.arena_mut(),
                        ctx,
                        ctx.localize(),
                        &func.params,
                        &values
                    ));
                    let instrs = func.table_rows.iter().flat_map(|row| row.block.iter());
                    let flow = backtrack!(instr::eval_sequential(
                        runner,
                        Cow::Owned(ctx_local),
                        instrs,
                        true
                    ));
                    match flow {
                        Flow::Return(_) => Backtrack::Ok(flow),
                        _ => invalid_flow(&id, "table did not return a value"),
                    }
                }
                ast::MetaFuncDef::Defined(func) => {
                    backtrack!(Backtrack::check(
                        func.tparams.len() == targs.len(),
                        id.span.clone(),
                        ErrorKind::Call(CallErrorKind::TypeArgumentArityMismatch {
                            expected: func.tparams.len(),
                            actual: targs.len()
                        })
                    ));
                    let mut ctx_local = ctx.localize();
                    for (tparam, targ) in func.tparams.iter().zip(targs.iter()) {
                        let def_typ = crate::phrase!(node: ast::DefTypKind::Plain(targ.clone()), span: targ.span.clone());
                        backtrack_from_result!(
                            ctx_local.bind_tparam(
                                tparam.clone(),
                                TypeDef::Defined(vec![], Box::new(def_typ))
                            ),
                            &tparam.span
                        );
                    }
                    let ctx_local = backtrack!(assign_params(
                        runner.arena_mut(),
                        ctx,
                        ctx_local,
                        &func.params,
                        &values
                    ));
                    let flow = backtrack!(instr::eval_body(
                        runner,
                        ctx_local,
                        &func.block,
                        func.block_else.as_deref()
                    ));
                    match flow {
                        Flow::Return(_) | Flow::TailFunc(..) => Backtrack::Ok(flow),
                        Flow::Cont(errors) => Backtrack::Unmatch(errors),
                        Flow::Result(_) => {
                            invalid_flow(&id, "function cannot produce a relation result")
                        }
                        Flow::TailRel(..) => {
                            invalid_flow(&id, "function cannot produce a relation tail call")
                        }
                    }
                }
            }
        });
        let pure = runner.interp_mut().cache.end();
        let result = result.nest(id.span.clone(), || ErrorKind::Trace(func_trace(&id, &targs)));
        let flow = match result {
            Backtrack::Ok(flow) => flow,
            Backtrack::Err(errors) => return nest_pending(Backtrack::Err(errors), pending),
            Backtrack::Unmatch(errors) => return nest_pending(Backtrack::Unmatch(errors), pending),
        };
        match flow {
            Flow::Return(value) => {
                if pure && let Some(key) = key {
                    runner.interp_mut().cache.funcs.insert(key, value);
                }
                return Backtrack::Ok(value);
            }
            Flow::TailFunc(id_tail, targs_tail, values_tail) => {
                let trace = func_trace(&id, &targs);
                pending.push((id.into_owned().span, trace));
                id = Cow::Owned(id_tail);
                targs = Cow::Owned(targs_tail);
                values = Cow::Owned(values_tail);
            }
            _ => unreachable!("function dispatch validates its flow"),
        }
    }
}

fn invalid_flow<T>(id: &ast::Id, message: &'static str) -> Backtrack<T> {
    Backtrack::err(id.span.clone(), ErrorKind::Call(CallErrorKind::InvalidFlow { message }))
}

fn assign_params<'global>(
    arena: &mut ValueArena,
    ctx_caller: &Context<'_>,
    mut ctx: Context<'global>,
    params: &[ast::Param],
    values: &[Value],
) -> Backtrack<Context<'global>> {
    backtrack!(Backtrack::check(
        params.len() == values.len(),
        crate::lang::common::source::Span::default(),
        ErrorKind::Assign(AssignErrorKind::ArgumentArityMismatch {
            expected: params.len(),
            actual: values.len()
        })
    ));
    for (param, value) in params.iter().zip(values) {
        match &param.node {
            ast::ParamKind::Exp(_, exp) => {
                ctx = backtrack!(assign::assign_exp(arena, ctx, exp, *value))
            }
            ast::ParamKind::Def(id, ..) => {
                let ValueKind::Func(id_func) = arena.kind(value) else {
                    return Backtrack::err(
                        id.span.clone(),
                        ErrorKind::Assign(AssignErrorKind::DefinitionMismatch {
                            value: arena.to_string(value),
                            def: id.node.clone(),
                        }),
                    );
                };
                let (_, func) =
                    backtrack_from_result!(ctx_caller.find_func(id_func), &id_func.span);
                backtrack_from_result!(ctx.add_func(id.clone(), Rc::clone(func)), &id.span);
            }
        }
    }
    Backtrack::Ok(ctx)
}

pub fn invoke_rel<Iface: Interface, Exn: Extern>(
    runner: &mut RunnerContext<'_, SlInterp, Iface, Exn>,
    ctx: &Context<'_>,
    id: &ast::Id,
    values: &[Value],
) -> Backtrack<Vec<Value>> {
    invoke_rel_mode(runner, ctx, id, values, true)
}
pub(crate) fn invoke_rel_entry<Iface: Interface, Exn: Extern>(
    runner: &mut RunnerContext<'_, SlInterp, Iface, Exn>,
    ctx: &Context<'_>,
    id: &ast::Id,
    values: &[Value],
) -> Backtrack<Vec<Value>> {
    invoke_rel_mode(runner, ctx, id, values, false)
}
pub fn invoke_func<Iface: Interface, Exn: Extern>(
    runner: &mut RunnerContext<'_, SlInterp, Iface, Exn>,
    ctx: &Context<'_>,
    id: &ast::Id,
    targs: &[ast::Typ],
    values: &[Value],
) -> Backtrack<Value> {
    invoke_func_mode(runner, ctx, id, targs, values, true)
}
pub(crate) fn invoke_func_entry<Iface: Interface, Exn: Extern>(
    runner: &mut RunnerContext<'_, SlInterp, Iface, Exn>,
    ctx: &Context<'_>,
    id: &ast::Id,
    targs: &[ast::Typ],
    values: &[Value],
) -> Backtrack<Value> {
    invoke_func_mode(runner, ctx, id, targs, values, false)
}

fn nest_pending<T>(
    mut result: Backtrack<T>,
    pending: Vec<(crate::lang::common::source::Span, TraceErrorKind)>,
) -> Backtrack<T> {
    for (span, trace) in pending.into_iter().rev() {
        result = result.nest(span, || ErrorKind::Trace(trace));
    }
    result
}

fn func_trace(id: &ast::Id, targs: &[ast::Typ]) -> TraceErrorKind {
    TraceErrorKind::FunctionInvocation {
        func: id.node.clone(),
        targs: if targs.is_empty() {
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
    }
}
