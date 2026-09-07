//! AL premise evaluation and iterative bindings

use super::super::{
    Al,
    backtrack::{Backtrack, back},
    context::Context,
};
use super::{assign, call::invoke_rel, expr};
use crate::{
    interp::common::Event,
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
use std::rc::Rc;

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
            let (exps_input, exps_output) = back!(Backtrack::from_result(
                input::split(&prem.input_hint, exps),
                &prem.id.span
            ));
            let values = back!(expr::eval_exps(runner, ctx, &exps_input));
            let values = back!(invoke_rel(runner, ctx, &prem.id, &values));
            assign::assign_exps(ctx, &exps_output, &values)
        }
        ast::PremKind::If(prem) => {
            let value = back!(expr::eval_exp(runner, ctx, &prem.exp));
            if back!(Backtrack::from_result(get::bool(&value), &prem.exp.span)) {
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
            let values = back!(expr::eval_exps(runner, ctx, &exps));
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
            let values = back!(expr::eval_exps(runner, ctx, &exps));
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
            let value = back!(expr::eval_exp(runner, ctx, &prem.exp_r));
            assign::assign_exp(ctx, &prem.exp_l, value)
        }
        ast::PremKind::Iter(prem) => {
            let iter = &prem.prem_iter;
            let ctxs = match iter.iter {
                ast::Iter::Opt => back!(Backtrack::from_result(
                    ctx.sub_opt(&iter.vars_bound),
                    &prem.prem.span
                ))
                .into_iter()
                .collect(),
                ast::Iter::List => back!(Backtrack::from_result(
                    ctx.sub_list(&iter.vars_bound),
                    &prem.prem.span
                )),
            };
            let mut batches = vec![Vec::new(); iter.vars_bind.len()];
            for ctx_sub in ctxs {
                let ctx_sub = back!(eval_prem(runner, &ctx_sub, &prem.prem));
                for (var, values) in iter.vars_bind.iter().zip(&mut batches) {
                    let variable = Variable::new(var.id.clone(), var.iters.clone());
                    values.push(Rc::clone(back!(Backtrack::from_result(
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
            let value = back!(expr::eval_exp(runner, ctx, &prem.exp));
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
