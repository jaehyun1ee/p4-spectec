//! PL expression, instruction, and call evaluation

use std::rc::Rc;

use crate::{
    interp::{
        pl::{
            PlInterp,
            context::{Context, Scope},
            flow::{self, Flow},
            prepare,
        },
        shared::{
            backtrack::{Backtrack, err, ok, unmatch, unwrap, unwrap_from_result},
            cache::CallKey,
            context::{IterContext, ReadContext, WriteContext},
            error::{
                AssignErrorKind, CallErrorKind, ErrorKind, GuardErrorKind, HostErrorKind,
                PremErrorKind, TraceErrorKind,
            },
            eval::{self as shared_eval, Invoker},
            prepare::{Prepare, ast as exec},
            util::iterate_vars,
        },
    },
    lang::{
        common::source::Span,
        data::value::{Value, ValueArena, ValueKind, get},
        pl::ast,
        traits::print::Print,
    },
    runner::{Extern, Interface, InterfaceError, RunnerContext},
    runtime::{envs::interp::shared::frame::FrameLayout, typdef::TypeDef},
};

fn eval_exp<Iface: Interface, Ext: Extern>(
    runner_ctx: &mut RunnerContext<'_, PlInterp, Iface, Ext>,
    ctx: &Context<'_>,
    exp: &ast::Exp,
) -> Backtrack<Value> {
    let exp_exec = prepare::prepare_exp(ctx.layout(), exp);
    shared_eval::expr::eval_exp(runner_ctx, ctx, &exp_exec)
}

fn eval_exps<Iface: Interface, Ext: Extern>(
    runner_ctx: &mut RunnerContext<'_, PlInterp, Iface, Ext>,
    ctx: &Context<'_>,
    exps: &[ast::Exp],
) -> Backtrack<Vec<Value>> {
    let exps_exec = prepare::prepare_exps(ctx.layout(), exps);
    shared_eval::expr::eval_exps(runner_ctx, ctx, &exps_exec)
}

fn assign_exp<'g>(
    arena: &mut crate::lang::data::value::ValueArena,
    ctx: Context<'g>,
    exp: &ast::Exp,
    value: Value,
) -> Backtrack<Context<'g>> {
    let exp_exec = prepare::prepare_exp(ctx.layout(), exp);
    shared_eval::assign::assign_exp(arena, ctx, &exp_exec, value)
}

fn assign_exps<'g>(
    arena: &mut crate::lang::data::value::ValueArena,
    ctx: Context<'g>,
    exps: &[ast::Exp],
    values: &[Value],
) -> Backtrack<Context<'g>> {
    let exps_exec = prepare::prepare_exps(ctx.layout(), exps);
    shared_eval::assign::assign_exps(arena, ctx, &exps_exec, values)
}

fn assign_params<'g>(
    arena: &mut crate::lang::data::value::ValueArena,
    ctx_caller: &Context<'_>,
    mut ctx: Context<'g>,
    params: &[ast::Param],
    values: &[Value],
) -> Backtrack<Context<'g>> {
    unwrap!(Backtrack::check(
        params.len() == values.len(),
        Span::default(),
        ErrorKind::Assign(AssignErrorKind::ArgumentArityMismatch {
            expected: params.len(),
            actual: values.len()
        })
    ));
    for (param, value) in params.iter().zip(values) {
        let result = match &param.node {
            ast::ParamKind::Exp(_, exp) => assign_exp(arena, ctx, exp, *value),
            ast::ParamKind::Def(id, ..) => {
                shared_eval::assign::assign_def(arena, ctx_caller, ctx, id, *value)
            }
        };
        ctx = unwrap!(result);
    }
    ok!(ctx)
}

fn prepare_iter(layout: &FrameLayout, iter: &ast::InstrIter) -> exec::PremIter {
    let len = layout.len();
    let mut layout = layout.clone();
    let iter = iter.clone().prepare(&mut layout);
    assert_eq!(layout.len(), len, "PL callable layout was not fully reserved");
    iter
}

fn prepare_exp_iter(layout: &FrameLayout, iter: &ast::ExpIter) -> exec::ExpIter {
    let len = layout.len();
    let mut layout = layout.clone();
    let iter = iter.clone().prepare(&mut layout);
    assert_eq!(layout.len(), len, "PL callable layout was not fully reserved");
    iter
}

fn eval_binding_iters<'g, Iface: Interface, Ext: Extern>(
    runner_ctx: &mut RunnerContext<'_, PlInterp, Iface, Ext>,
    ctx: Context<'g>,
    iters: &[ast::InstrIter],
    eval: &mut impl FnMut(
        &mut RunnerContext<'_, PlInterp, Iface, Ext>,
        Context<'g>,
    ) -> Backtrack<Context<'g>>,
) -> Backtrack<Context<'g>> {
    let Some((iter, rest)) = iters.split_last() else {
        return eval(runner_ctx, ctx);
    };
    let iter = prepare_iter(ctx.layout(), iter);
    shared_eval::iter::r#yield(runner_ctx, ctx, &Span::default(), &iter, |runner_ctx, ctx| {
        eval_binding_iters(runner_ctx, ctx, rest, eval)
    })
}

fn eval_condition_iters<Iface: Interface, Ext: Extern>(
    runner_ctx: &mut RunnerContext<'_, PlInterp, Iface, Ext>,
    ctx: &Context<'_>,
    iters: &[ast::ExpIter],
    eval: &mut impl FnMut(&mut RunnerContext<'_, PlInterp, Iface, Ext>, &Context<'_>) -> Backtrack<bool>,
) -> Backtrack<bool> {
    let Some((iter, rest)) = iters.split_last() else {
        return eval(runner_ctx, ctx);
    };
    let iter = prepare_exp_iter(ctx.layout(), iter);
    let vars_outer = iterate_vars(ctx, &iter.vars, iter.iter);
    match iter.iter {
        exec::Iter::Opt => {
            let values = unwrap_from_result!(
                ctx.find_opt_values_by_var(runner_ctx.arena(), &vars_outer),
                &Span::default()
            );
            let Some(values) = values else {
                return ok!(false);
            };
            let mut ctx_sub = ctx.clone();
            for (var, value) in iter.vars.iter().zip(values) {
                ctx_sub.add_value(var.slot, value);
            }
            eval_condition_iters(runner_ctx, &ctx_sub, rest, eval)
        }
        exec::Iter::List => {
            let values_by_var = unwrap_from_result!(
                ctx.find_list_values_by_var(runner_ctx.arena(), &vars_outer),
                &Span::default()
            );
            let values_by_var = values_by_var
                .into_iter()
                .map(<[Value]>::to_vec)
                .collect::<Vec<_>>();
            let len = values_by_var.first().map_or(0, Vec::len);
            let mut ctx_sub = ctx.clone();
            for idx in 0..len {
                for (var, values) in iter.vars.iter().zip(&values_by_var) {
                    ctx_sub.add_value(var.slot, values[idx]);
                }
                if !unwrap!(eval_condition_iters(runner_ctx, &ctx_sub, rest, eval)) {
                    return ok!(false);
                }
            }
            ok!(true)
        }
    }
}

fn eval_guard<'g, Iface: Interface, Ext: Extern>(
    runner_ctx: &mut RunnerContext<'_, PlInterp, Iface, Ext>,
    ctx: Context<'g>,
    value: Value,
    guard: &ast::Guard,
) -> Backtrack<Option<Context<'g>>> {
    let matched = match guard {
        ast::Guard::Bool(expected) => Backtrack::from_result(
            get::bool(runner_ctx.arena(), &value).map(|actual| actual == *expected),
            &Span::default(),
        ),
        ast::Guard::Cmp(op, _typ_op, exp) => {
            let value_r = unwrap!(eval_exp(runner_ctx, &ctx, exp));
            shared_eval::ops::cmpop(runner_ctx.arena(), &exp.node.span, op, value, value_r)
        }
        ast::Guard::Sub(_, check) | ast::Guard::CheckLetSub(_, check, _) => {
            shared_eval::ops::sub(runner_ctx.arena(), &ctx, &Span::default(), check, value)
        }
        ast::Guard::Match(pattern) | ast::Guard::CheckLetMatch(pattern, _) => {
            ok!(shared_eval::ops::r#match(runner_ctx.arena(), pattern, value))
        }
        ast::Guard::Mem(exp) => {
            let value_list = unwrap!(eval_exp(runner_ctx, &ctx, exp));
            shared_eval::ops::mem(runner_ctx.arena(), &exp.node.span, value, value_list)
        }
    };
    let matched = unwrap!(matched);
    if !matched {
        return ok!(None);
    }
    let ctx = match guard {
        ast::Guard::CheckLetSub(typ, _, exp) => {
            let value =
                unwrap!(shared_eval::ops::cast_down(runner_ctx.arena_mut(), &ctx, typ, value));
            unwrap!(assign_exp(runner_ctx.arena_mut(), ctx, exp, value))
        }
        ast::Guard::CheckLetMatch(_, exp) => {
            unwrap!(assign_exp(runner_ctx.arena_mut(), ctx, exp, value))
        }
        _ => ctx,
    };
    ok!(Some(ctx))
}

fn eval_common_instr<'g, Tier, Iface: Interface, Ext: Extern>(
    runner_ctx: &mut RunnerContext<'_, PlInterp, Iface, Ext>,
    ctx: Context<'g>,
    instr: &ast::Instr<Tier>,
    eval_tier: &mut impl FnMut(
        &mut RunnerContext<'_, PlInterp, Iface, Ext>,
        Context<'g>,
        &Tier,
    ) -> Backtrack<(Context<'g>, Flow)>,
    eval_block: &mut impl FnMut(
        &mut RunnerContext<'_, PlInterp, Iface, Ext>,
        Context<'g>,
        &ast::Block<Tier>,
    ) -> Backtrack<(Context<'g>, Flow)>,
) -> Backtrack<(Context<'g>, Flow)> {
    match &instr.node.node {
        ast::InstrKind::If(instr) => {
            let cond = unwrap!(eval_condition_iters(
                runner_ctx,
                &ctx,
                &instr.iter_exps,
                &mut |runner_ctx, ctx| {
                    let value = unwrap!(eval_exp(runner_ctx, ctx, &instr.exp));
                    Backtrack::from_result(
                        get::bool(runner_ctx.arena(), &value),
                        &instr.exp.node.span,
                    )
                },
            ));
            if cond {
                eval_block(runner_ctx, ctx, &instr.block)
            } else {
                ok!((
                    ctx,
                    Flow::cont(
                        instr.exp.node.span.clone(),
                        PremErrorKind::ConditionNotMet { exp: Print::to_string(&instr.exp) }
                    )
                ))
            }
        }
        ast::InstrKind::Hold(instr) => {
            let holds = unwrap!(eval_condition_iters(
                runner_ctx,
                &ctx,
                &instr.iter_exps,
                &mut |runner_ctx, ctx| {
                    let exps = instr
                        .not_exp
                        .args()
                        .into_iter()
                        .cloned()
                        .collect::<Vec<_>>();
                    let values = unwrap!(eval_exps(runner_ctx, ctx, &exps));
                    match PlInterp::invoke_rel(runner_ctx, ctx, &instr.id, &values) {
                        ok!(_) => ok!(true),
                        unmatch!(_) => ok!(false),
                        err!(errors) => err!(errors),
                    }
                },
            ));
            match &instr.hold_case {
                ast::HoldCase::Both(block_hold, block_not) => {
                    eval_block(runner_ctx, ctx, if holds { block_hold } else { block_not })
                }
                ast::HoldCase::Hold(block, _) if holds => eval_block(runner_ctx, ctx, block),
                ast::HoldCase::NotHold(block, _) if !holds => eval_block(runner_ctx, ctx, block),
                _ => ok!((
                    ctx,
                    Flow::cont(
                        instr.id.span.clone(),
                        PremErrorKind::ConditionNotMet { exp: instr.id.node.clone() }
                    )
                )),
            }
        }
        ast::InstrKind::Case(instr) => {
            let value = unwrap!(eval_exp(runner_ctx, &ctx, &instr.exp));
            for case in &instr.cases {
                if let Some(ctx_arm) =
                    unwrap!(eval_guard(runner_ctx, ctx.clone(), value, &case.guard))
                {
                    return eval_block(runner_ctx, ctx_arm, &case.block);
                }
            }
            ok!((
                ctx,
                Flow::cont(
                    instr.exp.node.span.clone(),
                    PremErrorKind::ConditionNotMet { exp: Print::to_string(&instr.exp) }
                )
            ))
        }
        ast::InstrKind::Let(instr) => {
            let ctx = unwrap!(eval_binding_iters(
                runner_ctx,
                ctx,
                &instr.iter_instrs,
                &mut |runner_ctx, ctx| {
                    let value = unwrap!(eval_exp(runner_ctx, &ctx, &instr.exp_r));
                    assign_exp(runner_ctx.arena_mut(), ctx, &instr.exp_l, value)
                }
            ));
            ok!((ctx, Flow::Cont(vec![])))
        }
        ast::InstrKind::Debug(instr) => {
            let value = unwrap!(eval_exp(runner_ctx, &ctx, &instr.exp));
            println!("{}", runner_ctx.arena().to_string(&value));
            ok!((ctx, Flow::Cont(vec![])))
        }
        ast::InstrKind::Destruct(instr) => {
            let value = unwrap!(eval_exp(runner_ctx, &ctx, &instr.exp));
            let values =
                unwrap_from_result!(get::case(runner_ctx.arena(), &value), &instr.exp.node.span)
                    .args()
                    .into_iter()
                    .copied()
                    .collect::<Vec<_>>();
            let exps = instr
                .bindings
                .iter()
                .map(|(_, exp)| exp.clone())
                .collect::<Vec<_>>();
            let ctx = unwrap!(assign_exps(runner_ctx.arena_mut(), ctx, &exps, &values));
            ok!((ctx, Flow::Cont(vec![])))
        }
        ast::InstrKind::CheckLetSub(instr) => {
            let value = unwrap!(eval_exp(runner_ctx, &ctx, &instr.exp_r));
            let matches = unwrap!(shared_eval::ops::sub(
                runner_ctx.arena(),
                &ctx,
                &instr.exp_r.node.span,
                &instr.subcheck,
                value
            ));
            if matches {
                let value = unwrap!(shared_eval::ops::cast_down(
                    runner_ctx.arena_mut(),
                    &ctx,
                    &instr.typ,
                    value
                ));
                match assign_exp(runner_ctx.arena_mut(), ctx.clone(), &instr.exp_l, value) {
                    ok!(ctx_bound) => eval_block(runner_ctx, ctx_bound, &instr.block),
                    err!(errors) | unmatch!(errors) => ok!((ctx, Flow::Cont(errors))),
                }
            } else {
                ok!((
                    ctx,
                    Flow::cont(
                        instr.exp_r.node.span.clone(),
                        PremErrorKind::ConditionNotMet {
                            exp: format!(
                                "{} is not a subtype of {}",
                                Print::to_string(&instr.exp_r),
                                Print::to_string(&instr.typ)
                            )
                        }
                    )
                ))
            }
        }
        ast::InstrKind::CheckLetMatch(instr) => {
            let value = unwrap!(eval_exp(runner_ctx, &ctx, &instr.exp_r));
            if shared_eval::ops::r#match(runner_ctx.arena(), &instr.pattern, value) {
                let ctx = unwrap!(assign_exp(runner_ctx.arena_mut(), ctx, &instr.exp_l, value));
                eval_block(runner_ctx, ctx, &instr.block)
            } else {
                ok!((
                    ctx,
                    Flow::cont(
                        instr.exp_r.node.span.clone(),
                        PremErrorKind::ConditionNotMet {
                            exp: format!(
                                "{} does not match the expected pattern",
                                Print::to_string(&instr.exp_r)
                            )
                        }
                    )
                ))
            }
        }
        ast::InstrKind::OptionGet(instr) => {
            let value = unwrap!(eval_exp(runner_ctx, &ctx, &instr.exp_r));
            if let Some(value) =
                unwrap_from_result!(get::opt(runner_ctx.arena(), &value), &instr.exp_r.node.span)
            {
                let ctx = unwrap!(assign_exp(runner_ctx.arena_mut(), ctx, &instr.exp_l, value));
                eval_block(runner_ctx, ctx, &instr.block)
            } else {
                ok!((
                    ctx,
                    Flow::cont(
                        instr.exp_r.node.span.clone(),
                        PremErrorKind::ConditionNotMet {
                            exp: format!(
                                "{} evaluated to an empty option",
                                Print::to_string(&instr.exp_r)
                            )
                        }
                    )
                ))
            }
        }
        ast::InstrKind::Tier(instr) => eval_tier(runner_ctx, ctx, &instr.tier),
    }
}

fn eval_group_block<'g, Iface: Interface, Ext: Extern>(
    runner_ctx: &mut RunnerContext<'_, PlInterp, Iface, Ext>,
    mut ctx: Context<'g>,
    block: &ast::GroupBlock,
) -> Backtrack<(Context<'g>, Flow)> {
    let mut errors = vec![];
    for instr in block {
        let (ctx_post, flow) = unwrap!(eval_group_instr(runner_ctx, ctx, instr));
        ctx = ctx_post;
        match flow {
            Flow::Cont(errors_post) => flow::retain_deepest_errors(&mut errors, errors_post),
            flow => return ok!((ctx, flow)),
        }
    }
    ok!((ctx, Flow::Cont(errors)))
}

fn eval_group_alternatives<'g, Iface: Interface, Ext: Extern>(
    runner_ctx: &mut RunnerContext<'_, PlInterp, Iface, Ext>,
    ctx: Context<'g>,
    blocks: &[ast::GroupBlock],
) -> Backtrack<(Context<'g>, Flow)> {
    if runner_ctx.interp().config.det {
        let mut selected = (ctx.clone(), Flow::Cont(vec![]));
        for block in blocks {
            let span = block
                .first()
                .map_or_else(Span::default, |instr| instr.node.span.clone());
            let (ctx_post, flow_post) = match eval_group_block(runner_ctx, ctx.clone(), block) {
                ok!(result) => result,
                unmatch!(_) => continue,
                err!(errors) => return err!(errors),
            };
            selected.1 = unwrap!(flow::combine_deterministic(selected.1, flow_post, &span));
            if !matches!(selected.1, Flow::Cont(_)) {
                selected.0 = ctx_post;
            }
        }
        ok!(selected)
    } else {
        let mut ctx = ctx;
        let mut errors = vec![];
        for block in blocks {
            match eval_group_block(runner_ctx, ctx.clone(), block) {
                ok!((ctx_post, Flow::Cont(errors_post))) => {
                    ctx = ctx_post;
                    flow::retain_deepest_errors(&mut errors, errors_post);
                }
                ok!(result) => return ok!(result),
                unmatch!(errors_post) => flow::retain_deepest_errors(&mut errors, errors_post),
                err!(errors) => return err!(errors),
            }
        }
        ok!((ctx, Flow::Cont(errors)))
    }
}

fn eval_group_instr<'g, Iface: Interface, Ext: Extern>(
    runner_ctx: &mut RunnerContext<'_, PlInterp, Iface, Ext>,
    ctx: Context<'g>,
    instr: &ast::Instr<ast::GroupInstr>,
) -> Backtrack<(Context<'g>, Flow)> {
    eval_common_instr(
        runner_ctx,
        ctx,
        instr,
        &mut |runner_ctx, ctx, tier| match tier {
            ast::GroupInstr::Result(instr) => {
                let values = unwrap!(eval_exps(runner_ctx, &ctx, &instr.exps_output));
                ok!((ctx, Flow::Result(values)))
            }
            ast::GroupInstr::Return(instr) => {
                let value = unwrap!(eval_exp(runner_ctx, &ctx, &instr.exp));
                ok!((ctx, Flow::Return(value)))
            }
            ast::GroupInstr::Rule(instr) => {
                let args = instr
                    .not_exp
                    .args()
                    .into_iter()
                    .cloned()
                    .collect::<Vec<_>>();
                let (exps_input, exps_output) = unwrap_from_result!(
                    crate::lang::hints::input::split(&instr.input_hint, args),
                    &instr.id.span
                );
                let ctx = unwrap!(eval_binding_iters(
                    runner_ctx,
                    ctx,
                    &instr.iter_instrs,
                    &mut |runner_ctx, ctx| {
                        let values = unwrap!(eval_exps(runner_ctx, &ctx, &exps_input));
                        let values =
                            unwrap!(PlInterp::invoke_rel(runner_ctx, &ctx, &instr.id, &values));
                        assign_exps(runner_ctx.arena_mut(), ctx, &exps_output, &values)
                    }
                ));
                ok!((ctx, Flow::Cont(vec![])))
            }
            ast::GroupInstr::Backtrack(instr) => {
                eval_group_alternatives(runner_ctx, ctx, &instr.blocks)
            }
        },
        &mut eval_group_block,
    )
    .nest(instr.node.span.clone(), || {
        ErrorKind::Trace(TraceErrorKind::Evaluation { text: Print::to_string(instr) })
    })
}

fn eval_dispatch_block<'g, Iface: Interface, Ext: Extern>(
    runner_ctx: &mut RunnerContext<'_, PlInterp, Iface, Ext>,
    mut ctx: Context<'g>,
    block: &ast::DispatchBlock,
) -> Backtrack<(Context<'g>, Flow)> {
    let mut errors = vec![];
    for instr in block {
        let (ctx_post, flow) = unwrap!(eval_dispatch_instr(runner_ctx, ctx, instr));
        ctx = ctx_post;
        match flow {
            Flow::Cont(errors_post) => flow::retain_deepest_errors(&mut errors, errors_post),
            flow => return ok!((ctx, flow)),
        }
    }
    ok!((ctx, Flow::Cont(errors)))
}

fn eval_dispatch_instr<'g, Iface: Interface, Ext: Extern>(
    runner_ctx: &mut RunnerContext<'_, PlInterp, Iface, Ext>,
    ctx: Context<'g>,
    instr: &ast::Instr<ast::DispatchInstr>,
) -> Backtrack<(Context<'g>, Flow)> {
    eval_common_instr(
        runner_ctx,
        ctx,
        instr,
        &mut |runner_ctx, ctx, tier| match tier {
            ast::DispatchInstr::Group(instr) => eval_group_block(runner_ctx, ctx, &instr.block),
            ast::DispatchInstr::Route(instr) => {
                if runner_ctx.interp().config.det {
                    let mut selected = (ctx.clone(), Flow::Cont(vec![]));
                    for block in &instr.blocks {
                        let span = block
                            .first()
                            .map_or_else(Span::default, |instr| instr.node.span.clone());
                        let (ctx_post, flow_post) =
                            match eval_dispatch_block(runner_ctx, ctx.clone(), block) {
                                ok!(result) => result,
                                unmatch!(_) => continue,
                                err!(errors) => return err!(errors),
                            };
                        selected.1 =
                            unwrap!(flow::combine_deterministic(selected.1, flow_post, &span));
                        if !matches!(selected.1, Flow::Cont(_)) {
                            selected.0 = ctx_post;
                        }
                    }
                    ok!(selected)
                } else {
                    let mut ctx = ctx;
                    let mut errors = vec![];
                    for block in &instr.blocks {
                        match eval_dispatch_block(runner_ctx, ctx.clone(), block) {
                            ok!((ctx_post, Flow::Cont(errors_post))) => {
                                ctx = ctx_post;
                                flow::retain_deepest_errors(&mut errors, errors_post);
                            }
                            ok!(result) => return ok!(result),
                            unmatch!(errors_post) => {
                                flow::retain_deepest_errors(&mut errors, errors_post)
                            }
                            err!(errors) => return err!(errors),
                        }
                    }
                    ok!((ctx, Flow::Cont(errors)))
                }
            }
        },
        &mut eval_dispatch_block,
    )
    .nest(instr.node.span.clone(), || {
        ErrorKind::Trace(TraceErrorKind::Evaluation { text: Print::to_string(instr) })
    })
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

pub(crate) fn check_rel_inputs(
    arena: &ValueArena,
    ctx: &Context<'_>,
    id: &ast::Id,
    values: &[Value],
) -> Backtrack<()> {
    let rel = unwrap_from_result!(ctx.find_rel(id), &id.span);
    let signature = match &rel.def {
        ast::RelDef::Extern(rel) => &rel.rel_signature,
        ast::RelDef::Defined(rel) => &rel.rel_signature,
    };
    let typs = signature.not_typ.node.args();
    unwrap_from_result!(
        crate::lang::hints::input::validate(&signature.input_hint, typs.len()),
        &id.span
    );
    let typs = signature
        .input_hint
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

pub(crate) fn check_func_inputs(
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
            actual: targs.len(),
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

pub(crate) fn cache_rel<Iface: Interface, Ext: Extern>(
    runner_ctx: &RunnerContext<'_, PlInterp, Iface, Ext>,
    ctx: &Context<'_>,
    id: &ast::Id,
) -> bool {
    runner_ctx.interp().config.cache
        && matches!(ctx.find_rel(id), Ok(rel) if matches!(&rel.def, ast::RelDef::Defined(_)))
}

pub(crate) fn cache_func<Iface: Interface, Ext: Extern>(
    runner_ctx: &RunnerContext<'_, PlInterp, Iface, Ext>,
    ctx: &Context<'_>,
    id: &ast::Id,
    values: &[Value],
) -> bool {
    runner_ctx.interp().config.cache
        && matches!(ctx.find_func_with_scope(id), Ok((Scope::Global, func))
            if !matches!(&func.def, ast::MetaFuncDef::Extern(_)))
        && !values
            .iter()
            .any(|value| matches!(runner_ctx.arena().kind(value), ValueKind::Func(_)))
}

fn invoke_extern_rel<Iface: Interface, Ext: Extern>(
    runner_ctx: &mut RunnerContext<'_, PlInterp, Iface, Ext>,
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
            .collect::<Vec<_>>();
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
            GuardErrorKind::RelationOutputMismatch { relation: id.node.clone() },
        ));
    }
    ok!(values)
}

fn invoke_defined_rel<Iface: Interface, Ext: Extern>(
    runner_ctx: &mut RunnerContext<'_, PlInterp, Iface, Ext>,
    ctx: &Context<'_>,
    layout: &Rc<FrameLayout>,
    id: &ast::Id,
    rel: &ast::DefinedRel,
    values: &[Value],
) -> Backtrack<Vec<Value>> {
    let ctx_local = unwrap!(assign_exps(
        runner_ctx.arena_mut(),
        ctx.localize_with_layout(layout),
        &rel.exps_input,
        values
    ));
    let mut flow = match eval_dispatch_block(runner_ctx, ctx_local.clone(), &rel.block) {
        ok!((_, flow)) => flow,
        unmatch!(errors) => Flow::Cont(errors),
        err!(errors) => return err!(errors),
    };
    if matches!(flow, Flow::Cont(_))
        && let Some(block) = &rel.block_else_opt
    {
        flow = unwrap!(eval_dispatch_block(runner_ctx, ctx_local, block)).1;
    }
    match flow {
        Flow::Result(values) => ok!(values),
        Flow::Cont(errors) => unmatch!(errors),
        Flow::Return(_) | Flow::TailFunc(..) | Flow::TailRel(..) => err!(
            id.span.clone(),
            ErrorKind::Call(CallErrorKind::InvalidFlow {
                message: "relation cannot return a value",
            })
        ),
    }
}

pub(crate) fn invoke_rel<Iface: Interface, Ext: Extern>(
    runner_ctx: &mut RunnerContext<'_, PlInterp, Iface, Ext>,
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
        let callable = unwrap_from_result!(ctx.find_rel(id), &id.span);
        match &callable.def {
            ast::RelDef::Extern(rel) => invoke_extern_rel(runner_ctx, ctx, id, rel, values),
            ast::RelDef::Defined(rel) => {
                invoke_defined_rel(runner_ctx, ctx, &callable.layout, id, rel, values)
            }
        }
    });
    let pure = runner_ctx.interp_mut().cache.end();
    let result = result.nest(id.span.clone(), || {
        ErrorKind::Trace(TraceErrorKind::Invocation { text: id.node.clone() })
    });
    let values = unwrap!(result);
    if pure && let Some(key) = key {
        runner_ctx
            .interp_mut()
            .cache
            .rels
            .insert(key, values.clone());
    }
    ok!(values)
}

fn invoke_extern_func<Iface: Interface, Ext: Extern>(
    runner_ctx: &mut RunnerContext<'_, PlInterp, Iface, Ext>,
    ctx: &Context<'_>,
    id: &ast::Id,
    func: &ast::ExternFunc,
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
            &func.tparams,
            &func.typ,
            targs,
            &value,
        ));
    }
    ok!(value)
}

fn invoke_builtin_func<Iface: Interface, Ext: Extern>(
    runner_ctx: &mut RunnerContext<'_, PlInterp, Iface, Ext>,
    ctx: &Context<'_>,
    id: &ast::Id,
    func: &ast::BuiltinFunc,
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
                    &func.tparams,
                    &func.typ,
                    targs,
                    &value,
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

fn invoke_table_func<Iface: Interface, Ext: Extern>(
    runner_ctx: &mut RunnerContext<'_, PlInterp, Iface, Ext>,
    ctx: &Context<'_>,
    layout: &Rc<FrameLayout>,
    id: &ast::Id,
    func: &ast::TableFunc,
    values: &[Value],
) -> Backtrack<Value> {
    let ctx_local = unwrap!(assign_params(
        runner_ctx.arena_mut(),
        ctx,
        ctx.localize_with_layout(layout),
        &func.params,
        values
    ));
    let block = func
        .rows
        .iter()
        .flat_map(|row| row.block.iter().cloned())
        .collect::<Vec<_>>();
    let (_, flow) = unwrap!(eval_group_block(runner_ctx, ctx_local, &block));
    match flow {
        Flow::Return(value) => ok!(value),
        _ => err!(
            id.span.clone(),
            ErrorKind::Call(CallErrorKind::InvalidFlow { message: "table did not return a value" })
        ),
    }
}

fn invoke_defined_func<Iface: Interface, Ext: Extern>(
    runner_ctx: &mut RunnerContext<'_, PlInterp, Iface, Ext>,
    ctx: &Context<'_>,
    layout: &Rc<FrameLayout>,
    id: &ast::Id,
    func: &ast::DefinedFunc,
    targs: &[ast::Typ],
    values: &[Value],
) -> Backtrack<Value> {
    unwrap!(Backtrack::check(
        func.tparams.len() == targs.len(),
        id.span.clone(),
        ErrorKind::Call(CallErrorKind::TypeArgumentArityMismatch {
            expected: func.tparams.len(),
            actual: targs.len(),
        })
    ));
    let mut ctx_local = ctx.localize_with_layout(layout);
    for (tparam, targ) in func.tparams.iter().zip(targs) {
        let def_typ =
            crate::phrase!(node: ast::DefTypKind::Plain(targ.clone()), span: targ.span.clone());
        unwrap_from_result!(
            ctx_local.bind_tparam(tparam.clone(), TypeDef::Defined(vec![], Box::new(def_typ))),
            &tparam.span
        );
    }
    let ctx_local =
        unwrap!(assign_params(runner_ctx.arena_mut(), ctx, ctx_local, &func.params, values));
    let mut flow = match eval_group_block(runner_ctx, ctx_local.clone(), &func.block) {
        ok!((_, flow)) => flow,
        unmatch!(errors) => Flow::Cont(errors),
        err!(errors) => return err!(errors),
    };
    if matches!(flow, Flow::Cont(_))
        && let Some(block) = &func.block_else_opt
    {
        flow = unwrap!(eval_group_block(runner_ctx, ctx_local, block)).1;
    }
    match flow {
        Flow::Return(value) => ok!(value),
        Flow::Cont(errors) => unmatch!(errors),
        Flow::Result(_) | Flow::TailFunc(..) | Flow::TailRel(..) => err!(
            id.span.clone(),
            ErrorKind::Call(CallErrorKind::InvalidFlow {
                message: "function cannot produce a relation result",
            })
        ),
    }
}

pub(crate) fn invoke_func<Iface: Interface, Ext: Extern>(
    runner_ctx: &mut RunnerContext<'_, PlInterp, Iface, Ext>,
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
        let func = unwrap_from_result!(ctx.find_func(id), &id.span);
        match &func.def {
            ast::MetaFuncDef::Extern(func) => {
                invoke_extern_func(runner_ctx, ctx, id, func, targs, values)
            }
            ast::MetaFuncDef::Builtin(func) => {
                invoke_builtin_func(runner_ctx, ctx, id, func, targs, values)
            }
            ast::MetaFuncDef::Table(func_def) => {
                invoke_table_func(runner_ctx, ctx, &func.layout, id, func_def, values)
            }
            ast::MetaFuncDef::Defined(func_def) => {
                invoke_defined_func(runner_ctx, ctx, &func.layout, id, func_def, targs, values)
            }
        }
    });
    let pure = runner_ctx.interp_mut().cache.end();
    let result =
        result.nest(id.span.clone(), || ErrorKind::Trace(TraceErrorKind::function(id, targs)));
    let value = unwrap!(result);
    if pure && let Some(key) = key {
        runner_ctx.interp_mut().cache.funcs.insert(key, value);
    }
    ok!(value)
}
