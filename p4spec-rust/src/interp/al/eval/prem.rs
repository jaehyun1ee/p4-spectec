//! AL premise evaluation and iterative bindings

use super::super::{AlInterp, context::Context};
use super::{assign, expr};
use crate::interp::shared::error::{PremErrorKind, TraceErrorKind};
use crate::interp::shared::{
    backtrack::{Backtrack, err, ok, unmatch, unwrap, unwrap_from_result},
    error::ErrorKind,
    eval::{Invoker, iter},
};
use crate::{
    lang::{al::ast, data::value::get, hints::input, traits::print::Print},
    runner::{Extern, Interface, RunnerContext},
};

// = Premise evaluation

pub fn eval_prem<'global, Iface: Interface, Ext: Extern>(
    runner_ctx: &mut RunnerContext<'_, AlInterp, Iface, Ext>,
    ctx: Context<'global>,
    prem: &ast::Prem,
) -> Backtrack<Context<'global>> {
    let result = match &prem.node {
        ast::PremKind::Rule(prem) => eval_rule_prem(runner_ctx, ctx, prem),
        ast::PremKind::If(prem) => eval_if_prem(runner_ctx, ctx, prem),
        ast::PremKind::IfHold(prem) => eval_if_hold_prem(runner_ctx, ctx, prem),
        ast::PremKind::IfNotHold(prem) => eval_if_not_hold_prem(runner_ctx, ctx, prem),
        ast::PremKind::Let(prem) => eval_let_prem(runner_ctx, ctx, prem),
        ast::PremKind::Iter(prem) => eval_iter_prem(runner_ctx, ctx, prem),
        ast::PremKind::Debug(prem) => eval_debug_prem(runner_ctx, ctx, prem),
    };
    result.nest(prem.span.clone(), || {
        ErrorKind::Trace(TraceErrorKind::Evaluation { text: Print::to_string(prem) })
    })
}

pub fn eval_prems<'global, Iface: Interface, Ext: Extern>(
    runner_ctx: &mut RunnerContext<'_, AlInterp, Iface, Ext>,
    mut ctx: Context<'global>,
    prems: &[ast::Prem],
) -> Backtrack<Context<'global>> {
    for prem in prems {
        ctx = unwrap!(eval_prem(runner_ctx, ctx, prem));
    }
    ok!(ctx)
}

// - Rule premise

fn eval_rule_prem<'global, Iface: Interface, Ext: Extern>(
    runner_ctx: &mut RunnerContext<'_, AlInterp, Iface, Ext>,
    ctx: Context<'global>,
    prem: &ast::RulePrem,
) -> Backtrack<Context<'global>> {
    let exps = prem.not_exp.args();
    let (exps_input, exps_output) =
        unwrap_from_result!(input::split(&prem.input_hint, exps), &prem.id.span);
    let values_input = unwrap!(expr::eval_exps(runner_ctx, &ctx, &exps_input));
    let values_output = unwrap!(AlInterp::invoke_rel(runner_ctx, &ctx, &prem.id, &values_input));
    assign::assign_exps(runner_ctx.arena_mut(), ctx, &exps_output, &values_output)
}

// - If premise

fn eval_if_prem<'global, Iface: Interface, Ext: Extern>(
    runner_ctx: &mut RunnerContext<'_, AlInterp, Iface, Ext>,
    ctx: Context<'global>,
    prem: &ast::IfPrem,
) -> Backtrack<Context<'global>> {
    let value = unwrap!(expr::eval_exp(runner_ctx, &ctx, &prem.exp));
    if unwrap_from_result!(get::bool(runner_ctx.arena(), &value), &prem.exp.span) {
        ok!(ctx)
    } else {
        unmatch!(
            prem.exp.span.clone(),
            ErrorKind::Prem(PremErrorKind::ConditionNotMet { exp: Print::to_string(&prem.exp) }),
        )
    }
}

// - Hold premise

fn eval_if_hold_prem<'global, Iface: Interface, Ext: Extern>(
    runner_ctx: &mut RunnerContext<'_, AlInterp, Iface, Ext>,
    ctx: Context<'global>,
    prem: &ast::IfHoldPrem,
) -> Backtrack<Context<'global>> {
    let exps: Vec<_> = prem.not_exp.args();
    let values = unwrap!(expr::eval_exps(runner_ctx, &ctx, &exps));
    match AlInterp::invoke_rel(runner_ctx, &ctx, &prem.id, &values) {
        ok!(_) => ok!(ctx),
        err!(errors) => err!(errors),
        unmatch!(errors) => unmatch!(errors).nest(prem.id.span.clone(), || {
            ErrorKind::Prem(PremErrorKind::HoldConditionNotMet { relation: prem.id.node.clone() })
        }),
    }
}

// - Not-hold premise

fn eval_if_not_hold_prem<'global, Iface: Interface, Ext: Extern>(
    runner_ctx: &mut RunnerContext<'_, AlInterp, Iface, Ext>,
    ctx: Context<'global>,
    prem: &ast::IfNotHoldPrem,
) -> Backtrack<Context<'global>> {
    let exps: Vec<_> = prem.not_exp.args();
    let values = unwrap!(expr::eval_exps(runner_ctx, &ctx, &exps));
    match AlInterp::invoke_rel(runner_ctx, &ctx, &prem.id, &values) {
        ok!(_) => unmatch!(
            prem.id.span.clone(),
            ErrorKind::Prem(PremErrorKind::NotHoldConditionNotMet {
                relation: prem.id.node.clone(),
            }),
        ),
        err!(errors) => err!(errors),
        unmatch!(_) => ok!(ctx),
    }
}

// - Let premise

fn eval_let_prem<'global, Iface: Interface, Ext: Extern>(
    runner_ctx: &mut RunnerContext<'_, AlInterp, Iface, Ext>,
    ctx: Context<'global>,
    prem: &ast::LetPrem,
) -> Backtrack<Context<'global>> {
    let value = unwrap!(expr::eval_exp(runner_ctx, &ctx, &prem.exp_r));
    assign::assign_exp(runner_ctx.arena_mut(), ctx, &prem.exp_l, value)
}

// - Iteration premise

fn eval_iter_prem<'global, Iface: Interface, Ext: Extern>(
    runner_ctx: &mut RunnerContext<'_, AlInterp, Iface, Ext>,
    ctx: Context<'global>,
    prem: &ast::IterPrem,
) -> Backtrack<Context<'global>> {
    let prem_iter = &prem.prem_iter;
    match prem_iter.iter {
        ast::Iter::Opt => iter::yield_opt(
            runner_ctx,
            ctx,
            &prem.prem.span,
            &prem_iter.vars_bound,
            &prem_iter.vars_bind,
            |runner_ctx, ctx_sub| eval_prem(runner_ctx, ctx_sub, &prem.prem),
        ),
        ast::Iter::List => iter::yield_list(
            runner_ctx,
            ctx,
            &prem.prem.span,
            &prem_iter.vars_bound,
            &prem_iter.vars_bind,
            |runner_ctx, ctx_sub| eval_prem(runner_ctx, ctx_sub, &prem.prem),
        ),
    }
}

// - Debug premise

fn eval_debug_prem<'global, Iface: Interface, Ext: Extern>(
    runner_ctx: &mut RunnerContext<'_, AlInterp, Iface, Ext>,
    ctx: Context<'global>,
    prem: &ast::DebugPrem,
) -> Backtrack<Context<'global>> {
    let value = unwrap!(expr::eval_exp(runner_ctx, &ctx, &prem.exp));
    let exp_text = Print::to_string(&prem.exp);
    println!("{}: {}", prem.exp.span, exp_text);
    let span_text = runner_ctx.arena().span(&value).to_string();
    if span_text.is_empty() {
        println!("{}", runner_ctx.arena().to_string(&value));
    } else {
        println!("{span_text}: {}", runner_ctx.arena().to_string(&value));
    }
    ok!(ctx)
}
