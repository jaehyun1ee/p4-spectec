//! Structured branch selection and explicit tail-call flow

use super::super::{
    SlInterp,
    context::{Context, Scope},
    flow::{self, Flow},
};
use super::{
    assign,
    expr::{self, eval_exp, eval_exps},
};
use crate::interp::shared::context::{IterContext, WriteContext};
use crate::interp::shared::eval::{Invoker, iter, ops};
use crate::interp::shared::util::iterate_vars;
use crate::runtime::envs::interp::sl::ast_prepared as ast;
use crate::{
    interp::shared::{
        backtrack::{Backtrack, err, ok, unmatch, unwrap, unwrap_from_result},
        error::{ErrorKind, PremErrorKind, TraceErrorKind},
    },
    lang::{
        common::source::Span,
        data::value::{Value, ValueKind, get},
        traits::{eq::SyntaxEq, print::Print},
    },
    runner::{Extern, Interface, RunnerContext},
};
use std::borrow::Cow;

// = Block evaluation

pub fn eval_block<Iface: Interface, Ext: Extern>(
    runner_ctx: &mut RunnerContext<'_, SlInterp, Iface, Ext>,
    ctx: Cow<'_, Context<'_>>,
    block: &[ast::Instr],
    tail: bool,
) -> Backtrack<Flow> {
    if runner_ctx.interp().config.det {
        eval_block_deterministic(runner_ctx, ctx.as_ref(), block, tail)
    } else {
        eval_block_sequential(runner_ctx, ctx, block.iter(), tail)
    }
}

pub(crate) fn eval_block_with_else<Iface: Interface, Ext: Extern>(
    runner_ctx: &mut RunnerContext<'_, SlInterp, Iface, Ext>,
    ctx: Context<'_>,
    block: &[ast::Instr],
    block_else: Option<&[ast::Instr]>,
) -> Backtrack<Flow> {
    let Some(block_else) = block_else else {
        return eval_block(runner_ctx, Cow::Owned(ctx), block, true);
    };
    let flow = unwrap!(eval_block(runner_ctx, Cow::Borrowed(&ctx), block, false));
    if matches!(flow, Flow::Cont(_)) {
        eval_block(runner_ctx, Cow::Owned(ctx), block_else, true)
    } else {
        ok!(flow)
    }
}

fn eval_block_deterministic<Iface: Interface, Ext: Extern>(
    runner_ctx: &mut RunnerContext<'_, SlInterp, Iface, Ext>,
    ctx: &Context<'_>,
    block: &[ast::Instr],
    tail: bool,
) -> Backtrack<Flow> {
    flow::choose_deterministic(
        block,
        |instr| eval_instr(runner_ctx, Cow::Borrowed(ctx), instr, tail),
        |instr| instr.span.clone(),
    )
}

pub(crate) fn eval_block_sequential<'instr, Iface: Interface, Ext: Extern>(
    runner_ctx: &mut RunnerContext<'_, SlInterp, Iface, Ext>,
    ctx: Cow<'_, Context<'_>>,
    instrs: impl DoubleEndedIterator<Item = &'instr ast::Instr>,
    tail: bool,
) -> Backtrack<Flow> {
    let mut ctx = Some(ctx);
    flow::choose_sequential(instrs, |instr, is_last| {
        if is_last {
            eval_instr(
                runner_ctx,
                ctx.take().expect("last instruction evaluated once"),
                instr,
                tail,
            )
        } else {
            eval_instr(
                runner_ctx,
                Cow::Borrowed(
                    ctx.as_ref()
                        .expect("last instruction not yet evaluated")
                        .as_ref(),
                ),
                instr,
                false,
            )
        }
    })
}

// = Instruction evaluation

pub fn eval_instr<Iface: Interface, Ext: Extern>(
    runner_ctx: &mut RunnerContext<'_, SlInterp, Iface, Ext>,
    ctx: Cow<'_, Context<'_>>,
    instr: &ast::Instr,
    tail: bool,
) -> Backtrack<Flow> {
    stacker::maybe_grow(64 * 1024, 1024 * 1024, || {
        let result = match &instr.node {
            ast::InstrKind::If(instr) => eval_if_instr(runner_ctx, ctx, instr, tail),
            ast::InstrKind::Hold(instr) => eval_hold_instr(runner_ctx, ctx, instr, tail),
            ast::InstrKind::Case(instr) => eval_case_instr(runner_ctx, ctx, instr, tail),
            ast::InstrKind::Group(instr) => eval_group_instr(runner_ctx, ctx, instr, tail),
            ast::InstrKind::Let(instr) => {
                Flow::cont_from_unmatch(eval_let_instr(runner_ctx, ctx, instr, tail))
            }
            ast::InstrKind::Rule(instr) => {
                Flow::cont_from_unmatch(eval_rule_instr(runner_ctx, ctx, instr, tail))
            }
            ast::InstrKind::Result(instr) => {
                Flow::cont_from_unmatch(eval_result_instr(runner_ctx, ctx, instr))
            }
            ast::InstrKind::Return(instr) => {
                Flow::cont_from_unmatch(eval_return_instr(runner_ctx, ctx, instr, tail))
            }
            ast::InstrKind::Debug(instr) => {
                Flow::cont_from_unmatch(eval_debug_instr(runner_ctx, ctx, instr, tail))
            }
        };
        result.nest(instr.span.clone(), || {
            ErrorKind::Trace(TraceErrorKind::Evaluation { text: Print::to_string(instr) })
        })
    })
}

// - If instruction

fn eval_if_instr<Iface: Interface, Ext: Extern>(
    runner_ctx: &mut RunnerContext<'_, SlInterp, Iface, Ext>,
    ctx: Cow<'_, Context<'_>>,
    instr: &ast::IfInstr,
    tail: bool,
) -> Backtrack<Flow> {
    let cond = unwrap!(eval_cond_iter(
        runner_ctx,
        ctx.as_ref(),
        &instr.iter_exps,
        &mut |runner_ctx, ctx| {
            let value = unwrap!(eval_exp(runner_ctx, ctx, &instr.exp));
            Backtrack::from_result(get::bool(runner_ctx.arena(), &value), &instr.exp.span)
        }
    ));
    if cond {
        eval_block(runner_ctx, ctx, &instr.block, tail)
    } else {
        ok!(Flow::cont(
            instr.exp.span.clone(),
            PremErrorKind::ConditionNotMet { exp: Print::to_string(&instr.exp) },
        ))
    }
}

// - Hold instruction

fn eval_hold_instr<Iface: Interface, Ext: Extern>(
    runner_ctx: &mut RunnerContext<'_, SlInterp, Iface, Ext>,
    ctx: Cow<'_, Context<'_>>,
    instr: &ast::HoldInstr,
    tail: bool,
) -> Backtrack<Flow> {
    let cond = unwrap!(eval_cond_iter(
        runner_ctx,
        ctx.as_ref(),
        &instr.iter_exps,
        &mut |runner_ctx, ctx| {
            let values = unwrap!(eval_exps(runner_ctx, ctx, &instr.not_exp.args()));
            match SlInterp::invoke_rel(runner_ctx, ctx, &instr.id, &values) {
                ok!(_) => ok!(true),
                unmatch!(_) => ok!(false),
                err!(errors) => err!(errors),
            }
        }
    ));
    match &instr.hold_case {
        ast::HoldCase::Both(block_hold, block_not) => {
            eval_block(runner_ctx, ctx, if cond { block_hold } else { block_not }, tail)
        }
        ast::HoldCase::Hold(block, _) if cond => eval_block(runner_ctx, ctx, block, tail),
        ast::HoldCase::NotHold(block, _) if !cond => eval_block(runner_ctx, ctx, block, tail),
        ast::HoldCase::Hold(..) => ok!(Flow::cont(
            instr.id.span.clone(),
            PremErrorKind::HoldConditionNotMet { relation: instr.id.node.clone() },
        )),
        ast::HoldCase::NotHold(..) => ok!(Flow::cont(
            instr.id.span.clone(),
            PremErrorKind::NotHoldConditionNotMet { relation: instr.id.node.clone() },
        )),
    }
}

// - Case instruction

fn eval_case_instr<Iface: Interface, Ext: Extern>(
    runner_ctx: &mut RunnerContext<'_, SlInterp, Iface, Ext>,
    ctx: Cow<'_, Context<'_>>,
    instr: &ast::CaseInstr,
    tail: bool,
) -> Backtrack<Flow> {
    let value = unwrap!(eval_exp(runner_ctx, ctx.as_ref(), &instr.exp));
    for case in &instr.cases {
        if unwrap!(eval_guard(runner_ctx, ctx.as_ref(), &instr.exp.span, value, &case.guard)) {
            return eval_block(runner_ctx, ctx, &case.block, tail);
        }
    }
    ok!(Flow::cont(
        instr.exp.span.clone(),
        PremErrorKind::ConditionNotMet { exp: format!("case {}", Print::to_string(&instr.exp)) },
    ))
}

fn eval_guard<Iface: Interface, Ext: Extern>(
    runner_ctx: &mut RunnerContext<'_, SlInterp, Iface, Ext>,
    ctx: &Context<'_>,
    span: &Span,
    value: Value,
    guard: &ast::Guard,
) -> Backtrack<bool> {
    if matches!(guard, ast::Guard::Bool(true)) {
        return Backtrack::from_result(get::bool(runner_ctx.arena(), &value), span);
    }
    (|| match guard {
        ast::Guard::Bool(_) => {
            ok!(!unwrap_from_result!(get::bool(runner_ctx.arena(), &value), span))
        }
        ast::Guard::Cmp(op, _, exp_r) => {
            let value_r = unwrap!(eval_exp(runner_ctx, ctx, exp_r));
            ops::cmpop(runner_ctx.arena(), span, op, value, value_r)
        }
        ast::Guard::Sub(_, check) => ops::sub(runner_ctx.arena(), ctx, span, check, value),
        ast::Guard::Match(pattern) => {
            ok!(ops::r#match(runner_ctx.arena(), pattern, value))
        }
        ast::Guard::Mem(exp_list) => {
            let value_list = unwrap!(eval_exp(runner_ctx, ctx, exp_list));
            ops::mem(runner_ctx.arena(), span, value, value_list)
        }
    })()
}

// - Group instruction

fn eval_group_instr<Iface: Interface, Ext: Extern>(
    runner_ctx: &mut RunnerContext<'_, SlInterp, Iface, Ext>,
    ctx: Cow<'_, Context<'_>>,
    instr: &ast::GroupInstr,
    tail: bool,
) -> Backtrack<Flow> {
    eval_block(runner_ctx, ctx, &instr.block, tail)
}

// - Let instruction

fn eval_let_instr<Iface: Interface, Ext: Extern>(
    runner_ctx: &mut RunnerContext<'_, SlInterp, Iface, Ext>,
    ctx: Cow<'_, Context<'_>>,
    instr: &ast::LetInstr,
    tail: bool,
) -> Backtrack<Flow> {
    let ctx = unwrap!(eval_instr_iter(
        runner_ctx,
        ctx.into_owned(),
        &instr.iter_instrs,
        &mut |runner_ctx, ctx| {
            let value = unwrap!(eval_exp(runner_ctx, &ctx, &instr.exp_r));
            assign::assign_exp(runner_ctx.arena_mut(), ctx, &instr.exp_l, value)
        }
    ));
    eval_block(runner_ctx, Cow::Owned(ctx), &instr.block, tail)
}

// - Rule instruction

fn eval_rule_instr<Iface: Interface, Ext: Extern>(
    runner_ctx: &mut RunnerContext<'_, SlInterp, Iface, Ext>,
    ctx: Cow<'_, Context<'_>>,
    instr: &ast::RuleInstr,
    tail: bool,
) -> Backtrack<Flow> {
    let (exps_input, exps_output) = unwrap_from_result!(
        crate::lang::hints::input::split(&instr.input_hint, instr.not_exp.args()),
        &instr.id.span
    );
    if tail
        && instr.iter_instrs.is_empty()
        && let [instr_result] = instr.block.as_slice()
        && let ast::InstrKind::Result(instr_result) = &instr_result.node
        && exps_output.len() == instr_result.exps.len()
        && exps_output
            .iter()
            .zip(&instr_result.exps)
            .all(|(exp_l, exp_r)| exp_l.syntax_eq(exp_r))
    {
        return ok!(Flow::TailRel(
            instr.id.clone(),
            unwrap!(eval_exps(runner_ctx, ctx.as_ref(), &exps_input)),
        ));
    }
    let ctx = unwrap!(eval_instr_iter(
        runner_ctx,
        ctx.into_owned(),
        &instr.iter_instrs,
        &mut |runner_ctx, ctx| {
            let values = unwrap!(eval_exps(runner_ctx, &ctx, &exps_input));
            let values = unwrap!(SlInterp::invoke_rel(runner_ctx, &ctx, &instr.id, &values));
            assign::assign_exps(runner_ctx.arena_mut(), ctx, &exps_output, &values)
        }
    ));
    eval_block(runner_ctx, Cow::Owned(ctx), &instr.block, tail)
}

// - Result instruction

fn eval_result_instr<Iface: Interface, Ext: Extern>(
    runner_ctx: &mut RunnerContext<'_, SlInterp, Iface, Ext>,
    ctx: Cow<'_, Context<'_>>,
    instr: &ast::ResultInstr,
) -> Backtrack<Flow> {
    ok!(Flow::Result(unwrap!(eval_exps(runner_ctx, ctx.as_ref(), &instr.exps))))
}

// - Return instruction

fn eval_return_instr<Iface: Interface, Ext: Extern>(
    runner_ctx: &mut RunnerContext<'_, SlInterp, Iface, Ext>,
    ctx: Cow<'_, Context<'_>>,
    instr: &ast::ReturnInstr,
    tail: bool,
) -> Backtrack<Flow> {
    if tail && let ast::ExpKind::Call(id, targs, args) = &instr.exp.node {
        let targs = unwrap_from_result!(expr::resolve_targs(ctx.as_ref(), targs), &id.span);
        let values = unwrap!(expr::eval_args(runner_ctx, ctx.as_ref(), args));
        let (scope, _) = unwrap_from_result!(ctx.find_func_with_scope(id), &id.span);
        if scope == Scope::Local
            || values
                .iter()
                .any(|value| matches!(runner_ctx.arena().kind(value), ValueKind::Func(_)))
        {
            ok!(Flow::Return(unwrap!(SlInterp::invoke_func(
                runner_ctx,
                ctx.as_ref(),
                id,
                &targs,
                &values
            ))))
        } else {
            ok!(Flow::TailFunc(id.clone(), targs, values))
        }
    } else {
        ok!(Flow::Return(unwrap!(eval_exp(runner_ctx, ctx.as_ref(), &instr.exp))))
    }
}

// - Debug instruction

fn eval_debug_instr<Iface: Interface, Ext: Extern>(
    runner_ctx: &mut RunnerContext<'_, SlInterp, Iface, Ext>,
    ctx: Cow<'_, Context<'_>>,
    instr: &ast::DebugInstr,
    tail: bool,
) -> Backtrack<Flow> {
    let value = unwrap!(eval_exp(runner_ctx, ctx.as_ref(), &instr.exp));
    println!("{}: {}", instr.exp.span, Print::to_string(&instr.exp));
    let span = runner_ctx.arena().span(&value).to_string();
    if span.is_empty() {
        println!("{}", runner_ctx.arena().to_string(&value));
    } else {
        println!("{span}: {}", runner_ctx.arena().to_string(&value));
    }
    eval_instr(runner_ctx, ctx, &instr.instr, tail)
}

// = Iteration

// - Condition iteration

fn eval_cond_iter<Iface: Interface, Ext: Extern>(
    runner_ctx: &mut RunnerContext<'_, SlInterp, Iface, Ext>,
    ctx: &Context<'_>,
    iters: &[ast::ExpIter],
    eval: &mut impl FnMut(&mut RunnerContext<'_, SlInterp, Iface, Ext>, &Context<'_>) -> Backtrack<bool>,
) -> Backtrack<bool> {
    let Some((exp_iter, iters_tail)) = iters.split_last() else {
        return eval(runner_ctx, ctx);
    };
    let ast::ExpIter { iter, vars } = exp_iter;
    let vars_outer = iterate_vars(ctx, vars, *iter);
    match iter {
        ast::Iter::Opt => {
            let values = unwrap_from_result!(
                ctx.find_opt_values_by_var(runner_ctx.arena(), &vars_outer),
                &Span::default()
            );
            let Some(values) = values else {
                return ok!(false);
            };
            let mut ctx_sub = ctx.clone();
            for (var, value) in vars.iter().zip(values) {
                ctx_sub.add_value(var.slot, value);
            }
            eval_cond_iter(runner_ctx, &ctx_sub, iters_tail, eval)
        }
        ast::Iter::List => {
            let values_by_var = unwrap_from_result!(
                ctx.find_list_values_by_var(runner_ctx.arena(), &vars_outer),
                &Span::default()
            );
            // Copy handles before the callback can allocate in the arena
            let values_by_var: Vec<_> = values_by_var.into_iter().map(<[Value]>::to_vec).collect();
            let len = values_by_var.first().map_or(0, Vec::len);
            let mut ctx_sub = ctx.clone();
            for idx in 0..len {
                for (var, values) in vars.iter().zip(&values_by_var) {
                    ctx_sub.add_value(var.slot, values[idx]);
                }
                if !unwrap!(eval_cond_iter(runner_ctx, &ctx_sub, iters_tail, eval)) {
                    return ok!(false);
                }
            }
            ok!(true)
        }
    }
}

// - Binding iteration

fn eval_instr_iter<'global, Iface: Interface, Ext: Extern>(
    runner_ctx: &mut RunnerContext<'_, SlInterp, Iface, Ext>,
    ctx: Context<'global>,
    iters: &[ast::InstrIter],
    eval: &mut impl FnMut(
        &mut RunnerContext<'_, SlInterp, Iface, Ext>,
        Context<'global>,
    ) -> Backtrack<Context<'global>>,
) -> Backtrack<Context<'global>> {
    let Some((iter, iters_tail)) = iters.split_last() else {
        return eval(runner_ctx, ctx);
    };
    iter::r#yield(runner_ctx, ctx, &Span::default(), iter, |runner_ctx, ctx_sub| {
        eval_instr_iter(runner_ctx, ctx_sub, iters_tail, eval)
    })
}
