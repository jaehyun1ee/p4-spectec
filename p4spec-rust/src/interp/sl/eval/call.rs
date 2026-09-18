//! SL invocation, memoization and tail-call dispatch

use super::super::{
    SlInterp,
    context::{Context, Scope},
    flow::Flow,
};
use super::{assign, instr};
use crate::lang::common::source::Span;
use crate::runtime::envs::interp::shared::frame::FrameLayout;
use crate::runtime::envs::interp::sl::ast_prepared as ast;
use crate::{
    interp::shared::{
        backtrack::{Backtrack, err, ok, unmatch, unwrap, unwrap_from_result},
        cache::CallKey,
        error::{CallErrorKind, ErrorKind, GuardErrorKind, HostErrorKind, TraceErrorKind},
    },
    lang::data::value::{Value, ValueArena, ValueKind},
    runner::{Extern, Interface, InterfaceError, RunnerContext},
    runtime::typdef::TypeDef,
};
use std::{borrow::Cow, rc::Rc};

// = Invocation results

enum FuncResult {
    Return(Value),
    TailCall(ast::Id, Vec<ast::Typ>, Vec<Value>),
}

enum RelResult {
    Result(Vec<Value>),
    TailCall(ast::Id, Vec<Value>),
}

// = Input and output checks

pub(in crate::interp::sl) fn check_rel_inputs(
    arena: &ValueArena,
    ctx: &Context<'_>,
    id: &ast::Id,
    values: &[Value],
) -> Backtrack<()> {
    let rel = unwrap_from_result!(ctx.find_rel(id), &id.span);
    let (not_typ, inputs) = match &rel.def {
        ast::RelDef::Extern(rel) => (&rel.rel_signature.not_typ, &rel.rel_signature.input_hint),
        ast::RelDef::Defined(rel) => (&rel.rel_signature.not_typ, &rel.rel_signature.input_hint),
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

pub(in crate::interp::sl) fn check_func_inputs(
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

pub(in crate::interp::sl) fn cache_rel<Iface: Interface, Ext: Extern>(
    runner_ctx: &RunnerContext<'_, SlInterp, Iface, Ext>,
    ctx: &Context<'_>,
    id: &ast::Id,
) -> bool {
    runner_ctx.interp().config.cache
        && matches!(ctx.find_rel(id), Ok(rel) if matches!(&rel.def, ast::RelDef::Defined(_)))
}

pub(in crate::interp::sl) fn cache_func<Iface: Interface, Ext: Extern>(
    runner_ctx: &RunnerContext<'_, SlInterp, Iface, Ext>,
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
    runner_ctx: &mut RunnerContext<'_, SlInterp, Iface, Ext>,
    ctx: &Context<'_>,
    id: &ast::Id,
    values: &[Value],
) -> Backtrack<Vec<Value>> {
    let mut id = Cow::Borrowed(id);
    let mut values = Cow::Borrowed(values);
    let mut traces_pending: Vec<(Span, TraceErrorKind)> = Vec::new();
    loop {
        let key = cache_rel(runner_ctx, ctx, &id)
            .then(|| CallKey::new(runner_ctx.arena(), &id.node, &values));
        if let Some(values) = key
            .as_ref()
            .and_then(|key| runner_ctx.interp().cache.rels.get(key))
        {
            return ok!(values.clone());
        }
        runner_ctx.interp_mut().cache.begin();
        let result = stacker::maybe_grow(64 * 1024, 1024 * 1024, || {
            let rel = unwrap_from_result!(ctx.find_rel(&id), &id.span);
            let layout = &rel.layout;
            match &rel.def {
                ast::RelDef::Extern(rel) => {
                    let values = unwrap!(invoke_extern_rel(runner_ctx, ctx, &id, rel, &values));
                    ok!(RelResult::Result(values))
                }
                ast::RelDef::Defined(rel) => {
                    invoke_defined_rel(runner_ctx, ctx, layout, &id, rel, &values)
                }
            }
        });
        let pure = runner_ctx.interp_mut().cache.end();
        let mut result = result.nest(id.span.clone(), || {
            ErrorKind::Trace(TraceErrorKind::Invocation { text: id.node.clone() })
        });
        if !matches!(result, ok!(_)) {
            for (span, trace) in traces_pending.iter().rev() {
                result = result.nest(span.clone(), || ErrorKind::Trace(trace.clone()));
            }
        }
        let result = unwrap!(result);
        match result {
            RelResult::Result(values) => {
                if pure && let Some(key) = key {
                    runner_ctx
                        .interp_mut()
                        .cache
                        .rels
                        .insert(key, values.clone());
                }
                return ok!(values);
            }
            RelResult::TailCall(id_tail, values_tail) => {
                let id_caller = id.into_owned();
                traces_pending
                    .push((id_caller.span, TraceErrorKind::Invocation { text: id_caller.node }));
                id = Cow::Owned(id_tail);
                values = Cow::Owned(values_tail);
            }
        }
    }
}

// - Extern relation

fn invoke_extern_rel<Iface: Interface, Ext: Extern>(
    runner_ctx: &mut RunnerContext<'_, SlInterp, Iface, Ext>,
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
        let typs = rel
            .rel_signature
            .not_typ
            .node
            .args()
            .into_iter()
            .cloned()
            .collect();
        let (_, typs) = unwrap_from_result!(
            crate::lang::hints::input::split(&rel.rel_signature.input_hint, typs),
            &id.span
        );
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

fn invoke_defined_rel<Iface: Interface, Ext: Extern>(
    runner_ctx: &mut RunnerContext<'_, SlInterp, Iface, Ext>,
    ctx: &Context<'_>,
    layout: &Rc<FrameLayout>,
    id: &ast::Id,
    rel: &ast::DefinedRel,
    values: &[Value],
) -> Backtrack<RelResult> {
    let ctx = unwrap!(assign::assign_exps(
        runner_ctx.arena_mut(),
        ctx.localize_with_layout(layout),
        &rel.exps_input,
        values
    ));
    let flow = unwrap!(instr::eval_block_with_else(
        runner_ctx,
        ctx,
        &rel.block,
        rel.block_else.as_deref()
    ));
    match flow {
        Flow::Result(values) => ok!(RelResult::Result(values)),
        Flow::TailRel(id, values) => ok!(RelResult::TailCall(id, values)),
        Flow::Cont(errors) => unmatch!(errors),
        Flow::Return(_) => err!(
            id.span.clone(),
            ErrorKind::Call(CallErrorKind::InvalidFlow {
                message: "relation cannot return a value",
            }),
        ),
        Flow::TailFunc(..) => err!(
            id.span.clone(),
            ErrorKind::Call(CallErrorKind::InvalidFlow {
                message: "unexpected function tailcall in relation body",
            }),
        ),
    }
}

// = Function invocation

pub fn invoke_func<Iface: Interface, Ext: Extern>(
    runner_ctx: &mut RunnerContext<'_, SlInterp, Iface, Ext>,
    ctx: &Context<'_>,
    id: &ast::Id,
    targs: &[ast::Typ],
    values: &[Value],
) -> Backtrack<Value> {
    let mut id = Cow::Borrowed(id);
    let mut targs = Cow::Borrowed(targs);
    let mut values = Cow::Borrowed(values);
    let mut traces_pending: Vec<(Span, TraceErrorKind)> = Vec::new();
    loop {
        let key = cache_func(runner_ctx, ctx, &id, &values)
            .then(|| CallKey::new(runner_ctx.arena(), &id.node, &values));
        if let Some(value) = key
            .as_ref()
            .and_then(|key| runner_ctx.interp().cache.funcs.get(key))
        {
            return ok!(*value);
        }
        runner_ctx.interp_mut().cache.begin();
        let result = stacker::maybe_grow(64 * 1024, 1024 * 1024, || {
            let (_, func) = unwrap_from_result!(ctx.find_func(&id), &id.span);
            let layout = &func.layout;
            match &func.def {
                ast::MetaFuncDef::Extern(func) => ok!(FuncResult::Return(unwrap!(
                    invoke_extern_func(runner_ctx, ctx, &id, func, &targs, &values)
                ))),
                ast::MetaFuncDef::Builtin(func) => ok!(FuncResult::Return(unwrap!(
                    invoke_builtin_func(runner_ctx, ctx, &id, func, &targs, &values)
                ))),
                ast::MetaFuncDef::Table(func) => {
                    invoke_table_func(runner_ctx, ctx, layout, &id, func, &values)
                }
                ast::MetaFuncDef::Defined(func) => {
                    invoke_defined_func(runner_ctx, ctx, layout, &id, func, &targs, &values)
                }
            }
        });
        let pure = runner_ctx.interp_mut().cache.end();
        let mut result = result
            .nest(id.span.clone(), || ErrorKind::Trace(TraceErrorKind::function(&id, &targs)));
        if !matches!(result, ok!(_)) {
            for (span, trace) in traces_pending.iter().rev() {
                result = result.nest(span.clone(), || ErrorKind::Trace(trace.clone()));
            }
        }
        let result = unwrap!(result);
        match result {
            FuncResult::Return(value) => {
                if pure && let Some(key) = key {
                    runner_ctx.interp_mut().cache.funcs.insert(key, value);
                }
                return ok!(value);
            }
            FuncResult::TailCall(id_tail, targs_tail, values_tail) => {
                let trace = TraceErrorKind::function(&id, &targs);
                traces_pending.push((id.into_owned().span, trace));
                id = Cow::Owned(id_tail);
                targs = Cow::Owned(targs_tail);
                values = Cow::Owned(values_tail);
            }
        }
    }
}

// - Extern function

fn invoke_extern_func<Iface: Interface, Ext: Extern>(
    runner_ctx: &mut RunnerContext<'_, SlInterp, Iface, Ext>,
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
    runner_ctx: &mut RunnerContext<'_, SlInterp, Iface, Ext>,
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

fn invoke_table_func<Iface: Interface, Ext: Extern>(
    runner_ctx: &mut RunnerContext<'_, SlInterp, Iface, Ext>,
    ctx: &Context<'_>,
    layout: &Rc<FrameLayout>,
    id: &ast::Id,
    func: &ast::TableFunc,
    values: &[Value],
) -> Backtrack<FuncResult> {
    let ctx_local = unwrap!(assign::assign_params(
        runner_ctx.arena_mut(),
        ctx,
        ctx.localize_with_layout(layout),
        &func.params,
        values
    ));
    let instrs = func.table_rows.iter().flat_map(|row| row.block.iter());
    let flow =
        unwrap!(instr::eval_block_sequential(runner_ctx, Cow::Owned(ctx_local), instrs, true));
    match flow {
        Flow::Return(value) => ok!(FuncResult::Return(value)),
        _ => err!(
            id.span.clone(),
            ErrorKind::Call(CallErrorKind::InvalidFlow { message: "table did not return a value" }),
        ),
    }
}

// - Defined function

fn invoke_defined_func<Iface: Interface, Ext: Extern>(
    runner_ctx: &mut RunnerContext<'_, SlInterp, Iface, Ext>,
    ctx: &Context<'_>,
    layout: &Rc<FrameLayout>,
    id: &ast::Id,
    func: &ast::DefinedFunc,
    targs: &[ast::Typ],
    values: &[Value],
) -> Backtrack<FuncResult> {
    unwrap!(Backtrack::check(
        func.tparams.len() == targs.len(),
        id.span.clone(),
        ErrorKind::Call(CallErrorKind::TypeArgumentArityMismatch {
            expected: func.tparams.len(),
            actual: targs.len()
        })
    ));
    let mut ctx_local = ctx.localize_with_layout(layout);
    for (tparam, targ) in func.tparams.iter().zip(targs.iter()) {
        let def_typ =
            crate::phrase!(node: ast::DefTypKind::Plain(targ.clone()), span: targ.span.clone());
        unwrap_from_result!(
            ctx_local.bind_tparam(tparam.clone(), TypeDef::Defined(vec![], Box::new(def_typ))),
            &tparam.span
        );
    }
    let ctx_local = unwrap!(assign::assign_params(
        runner_ctx.arena_mut(),
        ctx,
        ctx_local,
        &func.params,
        values
    ));
    let flow = unwrap!(instr::eval_block_with_else(
        runner_ctx,
        ctx_local,
        &func.block,
        func.block_else.as_deref()
    ));
    match flow {
        Flow::Return(value) => ok!(FuncResult::Return(value)),
        Flow::TailFunc(id, targs, values) => ok!(FuncResult::TailCall(id, targs, values)),
        Flow::Cont(errors) => unmatch!(errors),
        Flow::Result(_) => err!(
            id.span.clone(),
            ErrorKind::Call(CallErrorKind::InvalidFlow {
                message: "function cannot produce a relation result",
            }),
        ),
        Flow::TailRel(..) => err!(
            id.span.clone(),
            ErrorKind::Call(CallErrorKind::InvalidFlow {
                message: "function cannot produce a relation tail call",
            }),
        ),
    }
}
