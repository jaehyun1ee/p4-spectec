//! AL premise evaluation and iterative bindings

use super::super::{
    Al,
    backtrack::{Backtrack, back},
    context::Context,
    error::ErrorKind,
};
use super::{assign, call::invoke_rel, expr};
use crate::interp::al::error::PremErrorKind;
use crate::{
    lang::{al::ast, data::value::get, hints::input, traits::print::Print},
    runner::{Extern, Interface, RunnerContext},
};

// = Premise evaluation

pub fn eval_prem<'global, I: Interface, E: Extern>(
    runner: &mut RunnerContext<'_, Al, I, E>,
    ctx: Context<'global>,
    prem: &ast::Prem,
) -> Backtrack<Context<'global>> {
    match &prem.node {
        ast::PremKind::Rule(prem) => eval_rule_prem(runner, ctx, prem),
        ast::PremKind::If(prem) => eval_if_prem(runner, ctx, prem),
        ast::PremKind::IfHold(prem) => eval_if_hold_prem(runner, ctx, prem),
        ast::PremKind::IfNotHold(prem) => eval_if_not_hold_prem(runner, ctx, prem),
        ast::PremKind::Let(prem) => eval_let_prem(runner, ctx, prem),
        ast::PremKind::Iter(prem) => eval_iter_prem(runner, ctx, prem),
        ast::PremKind::Debug(prem) => eval_debug_prem(runner, ctx, prem),
    }
}

pub fn eval_prems<'global, I: Interface, E: Extern>(
    runner: &mut RunnerContext<'_, Al, I, E>,
    mut ctx: Context<'global>,
    prems: &[ast::Prem],
) -> Backtrack<Context<'global>> {
    for prem in prems {
        ctx = back!(eval_prem(runner, ctx, prem));
    }
    Backtrack::Ok(ctx)
}

// - Rule premise

fn eval_rule_prem<'global, I: Interface, E: Extern>(
    runner: &mut RunnerContext<'_, Al, I, E>,
    ctx: Context<'global>,
    prem: &ast::RulePrem,
) -> Backtrack<Context<'global>> {
    let exps = prem.not_exp.args();
    let (exps_input, exps_output) = back!(Backtrack::from_result(
        input::split(&prem.input_hint, exps),
        &prem.id.span
    ));
    let values_input = back!(expr::eval_exps(runner, &ctx, &exps_input));
    let values_output = back!(invoke_rel(runner, &ctx, &prem.id, &values_input));
    assign::assign_exps(runner.arena_mut(), ctx, &exps_output, &values_output)
}

// - If premise

fn eval_if_prem<'global, I: Interface, E: Extern>(
    runner: &mut RunnerContext<'_, Al, I, E>,
    ctx: Context<'global>,
    prem: &ast::IfPrem,
) -> Backtrack<Context<'global>> {
    let value = back!(expr::eval_exp(runner, &ctx, &prem.exp));
    if back!(Backtrack::from_result(
        get::bool(runner.arena(), &value),
        &prem.exp.span
    )) {
        Backtrack::Ok(ctx)
    } else {
        Backtrack::unmatch(
            prem.exp.span.clone(),
            ErrorKind::Prem(PremErrorKind::ConditionNotMet {
                expression: Print::to_string(&prem.exp),
            }),
        )
    }
}

// - Hold premise

fn eval_if_hold_prem<'global, I: Interface, E: Extern>(
    runner: &mut RunnerContext<'_, Al, I, E>,
    ctx: Context<'global>,
    prem: &ast::IfHoldPrem,
) -> Backtrack<Context<'global>> {
    let exps: Vec<_> = prem.not_exp.args();
    let values = back!(expr::eval_exps(runner, &ctx, &exps));
    match invoke_rel(runner, &ctx, &prem.id, &values) {
        Backtrack::Ok(_) => Backtrack::Ok(ctx),
        Backtrack::Err(errors) => Backtrack::Err(errors),
        Backtrack::Unmatch(errors) => Backtrack::Unmatch(errors).nest(prem.id.span.clone(), || {
            ErrorKind::Prem(PremErrorKind::HoldConditionNotMet {
                relation: prem.id.node.clone(),
            })
        }),
    }
}

// - Not-hold premise

fn eval_if_not_hold_prem<'global, I: Interface, E: Extern>(
    runner: &mut RunnerContext<'_, Al, I, E>,
    ctx: Context<'global>,
    prem: &ast::IfNotHoldPrem,
) -> Backtrack<Context<'global>> {
    let exps: Vec<_> = prem.not_exp.args();
    let values = back!(expr::eval_exps(runner, &ctx, &exps));
    match invoke_rel(runner, &ctx, &prem.id, &values) {
        Backtrack::Ok(_) => Backtrack::unmatch(
            prem.id.span.clone(),
            ErrorKind::Prem(PremErrorKind::NotHoldConditionNotMet {
                relation: prem.id.node.clone(),
            }),
        ),
        Backtrack::Err(errors) => Backtrack::Err(errors),
        Backtrack::Unmatch(_) => Backtrack::Ok(ctx),
    }
}

// - Let premise

fn eval_let_prem<'global, I: Interface, E: Extern>(
    runner: &mut RunnerContext<'_, Al, I, E>,
    ctx: Context<'global>,
    prem: &ast::LetPrem,
) -> Backtrack<Context<'global>> {
    let value = back!(expr::eval_exp(runner, &ctx, &prem.exp_r));
    assign::assign_exp(runner.arena_mut(), ctx, &prem.exp_l, value)
}

// - Iteration premise

fn eval_iter_prem<'global, I: Interface, E: Extern>(
    runner: &mut RunnerContext<'_, Al, I, E>,
    ctx: Context<'global>,
    prem: &ast::IterPrem,
) -> Backtrack<Context<'global>> {
    let prem_iter = &prem.prem_iter;
    match prem_iter.iter {
        ast::Iter::Opt => ctx.yield_opt(
            runner,
            &prem_iter.vars_bound,
            &prem_iter.vars_bind,
            &prem.prem.span,
            |runner, ctx_sub| eval_prem(runner, ctx_sub, &prem.prem),
        ),
        ast::Iter::List => ctx.yield_list(
            runner,
            &prem_iter.vars_bound,
            &prem_iter.vars_bind,
            &prem.prem.span,
            |runner, ctx_sub| eval_prem(runner, ctx_sub, &prem.prem),
        ),
    }
}

// - Debug premise

fn eval_debug_prem<'global, I: Interface, E: Extern>(
    runner: &mut RunnerContext<'_, Al, I, E>,
    ctx: Context<'global>,
    prem: &ast::DebugPrem,
) -> Backtrack<Context<'global>> {
    let value = back!(expr::eval_exp(runner, &ctx, &prem.exp));
    let exp_text = Print::to_string(&prem.exp);
    println!("{}: {}", prem.exp.span, exp_text);
    let span_text = runner.arena().span(&value).to_string();
    if span_text.is_empty() {
        println!("{}", runner.arena().to_string(&value));
    } else {
        println!("{span_text}: {}", runner.arena().to_string(&value));
    }
    Backtrack::Ok(ctx)
}
