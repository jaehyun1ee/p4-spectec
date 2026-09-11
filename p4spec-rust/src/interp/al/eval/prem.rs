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
    lang::{
        al::ast,
        common::{Variable, source::Span},
        data::{
            typ,
            value::{get, make},
        },
        hints::input,
        traits::print::Print,
    },
    runner::{Extern, Interface, RunnerContext},
};

// = Premise evaluation

pub fn eval_prem<'global, I: Interface, E: Extern>(
    runner: &mut RunnerContext<'_, Al, I, E>,
    ctx: &Context<'global>,
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
    ctx: &Context<'global>,
    prems: &[ast::Prem],
) -> Backtrack<Context<'global>> {
    let mut ctx = ctx.clone();
    for prem in prems {
        ctx = back!(eval_prem(runner, &ctx, prem));
    }
    Backtrack::Ok(ctx)
}

// - Rule premise

fn eval_rule_prem<'global, I: Interface, E: Extern>(
    runner: &mut RunnerContext<'_, Al, I, E>,
    ctx: &Context<'global>,
    prem: &ast::RulePrem,
) -> Backtrack<Context<'global>> {
    let exps = prem.not_exp.args().into_iter().cloned().collect();
    let (exps_input, exps_output) = back!(Backtrack::from_result(
        input::split(&prem.input_hint, exps),
        &prem.id.span
    ));
    let values_input = back!(expr::eval_exps(runner, ctx, &exps_input));
    let values_output = back!(invoke_rel(runner, ctx, &prem.id, &values_input));
    assign::assign_exps(runner.arena_mut(), ctx, &exps_output, &values_output)
}

// - If premise

fn eval_if_prem<'global, I: Interface, E: Extern>(
    runner: &mut RunnerContext<'_, Al, I, E>,
    ctx: &Context<'global>,
    prem: &ast::IfPrem,
) -> Backtrack<Context<'global>> {
    let value = back!(expr::eval_exp(runner, ctx, &prem.exp));
    if back!(Backtrack::from_result(
        get::bool(runner.arena(), &value),
        &prem.exp.span
    )) {
        Backtrack::Ok(ctx.clone())
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
    ctx: &Context<'global>,
    prem: &ast::IfHoldPrem,
) -> Backtrack<Context<'global>> {
    let exps: Vec<_> = prem.not_exp.args().into_iter().cloned().collect();
    let values = back!(expr::eval_exps(runner, ctx, &exps));
    match invoke_rel(runner, ctx, &prem.id, &values) {
        Backtrack::Ok(_) => Backtrack::Ok(ctx.clone()),
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
    ctx: &Context<'global>,
    prem: &ast::IfNotHoldPrem,
) -> Backtrack<Context<'global>> {
    let exps: Vec<_> = prem.not_exp.args().into_iter().cloned().collect();
    let values = back!(expr::eval_exps(runner, ctx, &exps));
    match invoke_rel(runner, ctx, &prem.id, &values) {
        Backtrack::Ok(_) => Backtrack::unmatch(
            prem.id.span.clone(),
            ErrorKind::Prem(PremErrorKind::NotHoldConditionNotMet {
                relation: prem.id.node.clone(),
            }),
        ),
        Backtrack::Err(errors) => Backtrack::Err(errors),
        Backtrack::Unmatch(_) => Backtrack::Ok(ctx.clone()),
    }
}

// - Let premise

fn eval_let_prem<'global, I: Interface, E: Extern>(
    runner: &mut RunnerContext<'_, Al, I, E>,
    ctx: &Context<'global>,
    prem: &ast::LetPrem,
) -> Backtrack<Context<'global>> {
    let value = back!(expr::eval_exp(runner, ctx, &prem.exp_r));
    assign::assign_exp(runner.arena_mut(), ctx, &prem.exp_l, value)
}

// - Iteration premise

fn eval_iter_prem<'global, I: Interface, E: Extern>(
    runner: &mut RunnerContext<'_, Al, I, E>,
    ctx: &Context<'global>,
    prem: &ast::IterPrem,
) -> Backtrack<Context<'global>> {
    let prem_iter = &prem.prem_iter;
    let ctxs = match prem_iter.iter {
        ast::Iter::Opt => back!(Backtrack::from_result(
            ctx.sub_opt(runner.arena(), &prem_iter.vars_bound),
            &prem.prem.span
        ))
        .into_iter()
        .collect(),
        ast::Iter::List => back!(Backtrack::from_result(
            ctx.sub_list(runner.arena(), &prem_iter.vars_bound),
            &prem.prem.span
        )),
    };
    let mut values_bind = vec![Vec::new(); prem_iter.vars_bind.len()];
    for ctx_sub in ctxs {
        let ctx_sub = back!(eval_prem(runner, &ctx_sub, &prem.prem));
        for (var, values) in prem_iter.vars_bind.iter().zip(&mut values_bind) {
            let var_bound = Variable::new(var.id.clone(), var.iters.clone());
            values.push(
                *(back!(Backtrack::from_result(
                    ctx_sub.find_value(&var_bound),
                    &var.id.span
                ))),
            );
        }
    }
    let mut ctx = ctx.clone();
    for (var, values) in prem_iter.vars_bind.iter().zip(values_bind) {
        let mut iters = var.iters.clone();
        iters.push(prem_iter.iter);
        let typ = typ::make::iterate(var.typ.clone(), &iters);
        let value = match prem_iter.iter {
            ast::Iter::Opt => back!(Backtrack::from_result(
                make::opt(
                    runner.arena_mut(),
                    typ.node.clone(),
                    values.into_iter().next(),
                    Span::default()
                ),
                &Span::default()
            )),
            ast::Iter::List => back!(Backtrack::from_result(
                make::list(
                    runner.arena_mut(),
                    typ.node.clone(),
                    values,
                    Span::default()
                ),
                &Span::default()
            )),
        };
        ctx.add_value(Variable::new(var.id.clone(), iters), value);
    }
    Backtrack::Ok(ctx)
}

// - Debug premise

fn eval_debug_prem<'global, I: Interface, E: Extern>(
    runner: &mut RunnerContext<'_, Al, I, E>,
    ctx: &Context<'global>,
    prem: &ast::DebugPrem,
) -> Backtrack<Context<'global>> {
    let value = back!(expr::eval_exp(runner, ctx, &prem.exp));
    let exp_text = Print::to_string(&prem.exp);
    println!("{}: {}", prem.exp.span, exp_text);
    let span_text = runner.arena().span(&value).to_string();
    if span_text.is_empty() {
        println!("{}", runner.arena().to_string(&value));
    } else {
        println!("{span_text}: {}", runner.arena().to_string(&value));
    }
    Backtrack::Ok(ctx.clone())
}
