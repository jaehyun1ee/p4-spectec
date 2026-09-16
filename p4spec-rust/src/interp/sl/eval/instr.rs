//! Structured branch selection and explicit tail-call flow

use super::super::{
    SlInterp,
    context::{Context, Scope},
};
use super::{
    assign,
    call::{invoke_func, invoke_rel},
    expr::{self, eval_exp, eval_exps},
};
use crate::interp::shared::ops;
use crate::{
    interp::shared::{
        backtrack::{Backtrack, backtrack, backtrack_from_result},
        error::{CallErrorKind, Error, ErrorKind, PremErrorKind, TraceErrorKind},
    },
    lang::{
        common::{Variable, source::Span},
        data::value::{Value, ValueKind, get},
        sl::ast,
        traits::{eq::SyntaxEq, print::Print},
        xl::bool as boolean,
    },
    runner::{Extern, Interface, RunnerContext},
};
use std::borrow::Cow;

#[derive(Clone, Debug)]
pub enum Flow {
    Cont(Vec<Error>),
    Return(Value),
    Result(Vec<Value>),
    TailFunc(ast::Id, Vec<ast::Typ>, Vec<Value>),
    TailRel(ast::Id, Vec<Value>),
}

pub(crate) fn eval_body<Iface: Interface, Exn: Extern>(
    runner: &mut RunnerContext<'_, SlInterp, Iface, Exn>,
    ctx: Context<'_>,
    block: &[ast::Instr],
    block_else: Option<&[ast::Instr]>,
) -> Backtrack<Flow> {
    let Some(block_else) = block_else else {
        return eval_block_ctx(runner, Cow::Owned(ctx), block, true);
    };
    let flow = backtrack!(eval_block(runner, &ctx, block, false));
    if matches!(flow, Flow::Cont(_)) {
        eval_block_ctx(runner, Cow::Owned(ctx), block_else, true)
    } else {
        Backtrack::Ok(flow)
    }
}

pub fn eval_block<Iface: Interface, Exn: Extern>(
    runner: &mut RunnerContext<'_, SlInterp, Iface, Exn>,
    ctx: &Context<'_>,
    block: &[ast::Instr],
    tail: bool,
) -> Backtrack<Flow> {
    eval_block_ctx(runner, Cow::Borrowed(ctx), block, tail)
}

fn eval_block_ctx<Iface: Interface, Exn: Extern>(
    runner: &mut RunnerContext<'_, SlInterp, Iface, Exn>,
    ctx: Cow<'_, Context<'_>>,
    block: &[ast::Instr],
    tail: bool,
) -> Backtrack<Flow> {
    if !runner.interp().config.det {
        return eval_sequential(runner, ctx, block.iter(), tail);
    }
    let mut flow = Flow::Cont(vec![]);
    for instr in block {
        let flow_post = match eval_instr(runner, ctx.as_ref(), instr, tail) {
            Backtrack::Unmatch(_) => continue,
            result => backtrack!(result),
        };
        flow = match (flow, flow_post) {
            (Flow::Cont(mut errors), Flow::Cont(errors_post)) => {
                errors.extend(errors_post);
                Flow::Cont(errors)
            }
            (Flow::Cont(_), flow) | (flow, Flow::Cont(_)) => flow,
            (Flow::Return(_), Flow::Return(_))
            | (Flow::Result(_), Flow::Result(_))
            | (Flow::TailFunc(..) | Flow::TailRel(..), Flow::TailFunc(..) | Flow::TailRel(..)) => {
                return Backtrack::err(
                    instr.span.clone(),
                    ErrorKind::Call(CallErrorKind::InstructionNondeterminism),
                );
            }
            (flow_pre, flow_post) => {
                let message = match (flow_pre, flow_post) {
                    (Flow::Result(_), Flow::Return(_)) => "cannot have both result and return",
                    (Flow::Result(_), _) => "cannot have both result and tail call",
                    (Flow::Return(_), Flow::Result(_)) => "cannot have both return and result",
                    (Flow::Return(_), _) => "cannot have both return and tail call",
                    (Flow::TailFunc(..), Flow::Result(_)) => {
                        "cannot have both tail call and result"
                    }
                    (Flow::TailFunc(..), _) => "cannot have both tail call and return",
                    (Flow::TailRel(..), Flow::Result(_)) => {
                        "cannot have both rel tail call and result"
                    }
                    (Flow::TailRel(..), _) => "cannot have both rel tail call and return",
                    (Flow::Cont(_), _) => unreachable!("continuations were combined above"),
                };
                return Backtrack::err(
                    instr.span.clone(),
                    ErrorKind::Call(CallErrorKind::InvalidFlow { message }),
                );
            }
        };
    }
    Backtrack::Ok(flow)
}

pub(crate) fn eval_sequential<'instr, Iface: Interface, Exn: Extern>(
    runner: &mut RunnerContext<'_, SlInterp, Iface, Exn>,
    ctx: Cow<'_, Context<'_>>,
    mut instrs: impl DoubleEndedIterator<Item = &'instr ast::Instr>,
    tail: bool,
) -> Backtrack<Flow> {
    let Some(instr_last) = instrs.next_back() else {
        return Backtrack::Ok(Flow::Cont(vec![]));
    };
    let mut errors = Vec::new();
    for instr in instrs {
        match backtrack!(eval_instr(runner, ctx.as_ref(), instr, false)) {
            Flow::Cont(errors_post) => retain_errors(&mut errors, errors_post),
            flow => return Backtrack::Ok(flow),
        }
    }
    match backtrack!(eval_instr_ctx(runner, ctx, instr_last, tail)) {
        Flow::Cont(errors_post) => {
            retain_errors(&mut errors, errors_post);
            Backtrack::Ok(Flow::Cont(errors))
        }
        flow => Backtrack::Ok(flow),
    }
}

fn retain_errors(errors: &mut Vec<Error>, errors_post: Vec<Error>) {
    if errors_post.iter().map(Error::depth).max().unwrap_or(0)
        >= errors.iter().map(Error::depth).max().unwrap_or(0)
    {
        *errors = errors_post;
    }
}

pub fn eval_instr<Iface: Interface, Exn: Extern>(
    runner: &mut RunnerContext<'_, SlInterp, Iface, Exn>,
    ctx: &Context<'_>,
    instr: &ast::Instr,
    tail: bool,
) -> Backtrack<Flow> {
    eval_instr_ctx(runner, Cow::Borrowed(ctx), instr, tail)
}

fn eval_instr_ctx<Iface: Interface, Exn: Extern>(
    runner: &mut RunnerContext<'_, SlInterp, Iface, Exn>,
    ctx: Cow<'_, Context<'_>>,
    instr: &ast::Instr,
    tail: bool,
) -> Backtrack<Flow> {
    stacker::maybe_grow(64 * 1024, 1024 * 1024, || eval_instr_inner(runner, ctx, instr, tail))
}

fn eval_instr_inner<Iface: Interface, Exn: Extern>(
    runner: &mut RunnerContext<'_, SlInterp, Iface, Exn>,
    ctx: Cow<'_, Context<'_>>,
    instr: &ast::Instr,
    tail: bool,
) -> Backtrack<Flow> {
    let result = (|| match &instr.node {
        ast::InstrKind::If(instr) => {
            let cond = backtrack!(condition_iter(
                runner,
                ctx.as_ref(),
                &instr.iter_exps,
                true,
                &mut |runner, ctx| {
                    let value = backtrack!(eval_exp(runner, ctx, &instr.exp));
                    Backtrack::from_result(get::bool(runner.arena(), &value), &instr.exp.span)
                }
            ));
            if cond {
                eval_block_ctx(runner, ctx, &instr.block, tail)
            } else {
                continuation(
                    instr.exp.span.clone(),
                    PremErrorKind::ConditionNotMet { exp: Print::to_string(&instr.exp) },
                )
            }
        }
        ast::InstrKind::Hold(instr) => {
            let cond = backtrack!(condition_iter(
                runner,
                ctx.as_ref(),
                &instr.iter_exps,
                true,
                &mut |runner, ctx| {
                    let values = backtrack!(eval_exps(runner, ctx, &instr.not_exp.args()));
                    match invoke_rel(runner, ctx, &instr.id, &values) {
                        Backtrack::Ok(_) => Backtrack::Ok(true),
                        Backtrack::Unmatch(_) => Backtrack::Ok(false),
                        Backtrack::Err(errors) => Backtrack::Err(errors),
                    }
                }
            ));
            match &instr.hold_case {
                ast::HoldCase::Both(block_hold, block_not) => {
                    eval_block_ctx(runner, ctx, if cond { block_hold } else { block_not }, tail)
                }
                ast::HoldCase::Hold(block, _) if cond => eval_block_ctx(runner, ctx, block, tail),
                ast::HoldCase::NotHold(block, _) if !cond => {
                    eval_block_ctx(runner, ctx, block, tail)
                }
                ast::HoldCase::Hold(..) => continuation(
                    instr.id.span.clone(),
                    PremErrorKind::HoldConditionNotMet { relation: instr.id.node.clone() },
                ),
                ast::HoldCase::NotHold(..) => continuation(
                    instr.id.span.clone(),
                    PremErrorKind::NotHoldConditionNotMet { relation: instr.id.node.clone() },
                ),
            }
        }
        ast::InstrKind::Case(instr) => {
            let value = backtrack!(eval_exp(runner, ctx.as_ref(), &instr.exp));
            let id = crate::phrase!(node: "~case".to_owned(), span: Span::default());
            let mut ctx_guard = ctx.as_ref().clone();
            ctx_guard.add_value(Variable::new(id.clone(), vec![]), value);
            let exp = crate::note_phrase!(node: ast::ExpKind::Var(id), note: instr.exp.note.clone(), span: instr.exp.span.clone());
            for case in &instr.cases {
                if backtrack!(eval_guard(runner, &ctx_guard, &exp, value, &case.guard)) {
                    drop(ctx_guard);
                    return eval_block_ctx(runner, ctx, &case.block, tail);
                }
            }
            continuation(
                instr.exp.span.clone(),
                PremErrorKind::ConditionNotMet {
                    exp: format!("case {}", Print::to_string(&instr.exp)),
                },
            )
        }
        ast::InstrKind::Group(instr) => eval_block_ctx(runner, ctx, &instr.block, tail),
        ast::InstrKind::Let(instr) => {
            let ctx = backtrack!(binding_iter(
                runner,
                ctx.into_owned(),
                &instr.iter_instrs,
                &mut |runner, ctx| {
                    let value = backtrack!(eval_exp(runner, &ctx, &instr.exp_r));
                    assign::assign_exp(runner.arena_mut(), ctx, &instr.exp_l, value)
                }
            ));
            eval_block_ctx(runner, Cow::Owned(ctx), &instr.block, tail)
        }
        ast::InstrKind::Rule(instr) => {
            let (exps_input, exps_output) = backtrack_from_result!(
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
                return Backtrack::Ok(Flow::TailRel(
                    instr.id.clone(),
                    backtrack!(eval_exps(runner, ctx.as_ref(), &exps_input)),
                ));
            }
            let ctx = backtrack!(binding_iter(
                runner,
                ctx.into_owned(),
                &instr.iter_instrs,
                &mut |runner, ctx| {
                    let values = backtrack!(eval_exps(runner, &ctx, &exps_input));
                    let values = backtrack!(invoke_rel(runner, &ctx, &instr.id, &values));
                    assign::assign_exps(runner.arena_mut(), ctx, &exps_output, &values)
                }
            ));
            eval_block_ctx(runner, Cow::Owned(ctx), &instr.block, tail)
        }
        ast::InstrKind::Result(instr) => {
            Backtrack::Ok(Flow::Result(backtrack!(eval_exps(runner, ctx.as_ref(), &instr.exps))))
        }
        ast::InstrKind::Return(instr) => {
            if tail && let ast::ExpKind::Call(id, targs, args) = &instr.exp.node {
                let theta = ctx.theta_local();
                let targs = backtrack_from_result!(
                    targs
                        .iter()
                        .map(|targ| crate::runtime::ops::typ::subst_typ(&theta, targ))
                        .collect::<Result<Vec<_>, _>>(),
                    &id.span
                );
                let values = backtrack!(expr::eval_args(runner, ctx.as_ref(), args));
                let (scope, _) = backtrack_from_result!(ctx.find_func(id), &id.span);
                if scope == Scope::Local
                    || values
                        .iter()
                        .any(|value| matches!(runner.arena().kind(value), ValueKind::Func(_)))
                {
                    Backtrack::Ok(Flow::Return(backtrack!(invoke_func(
                        runner,
                        ctx.as_ref(),
                        id,
                        &targs,
                        &values
                    ))))
                } else {
                    Backtrack::Ok(Flow::TailFunc(id.clone(), targs, values))
                }
            } else {
                Backtrack::Ok(Flow::Return(backtrack!(eval_exp(runner, ctx.as_ref(), &instr.exp))))
            }
        }
        ast::InstrKind::Debug(instr) => {
            let value = backtrack!(eval_exp(runner, ctx.as_ref(), &instr.exp));
            println!("{}: {}", instr.exp.span, Print::to_string(&instr.exp));
            let span = runner.arena().span(&value).to_string();
            if span.is_empty() {
                println!("{}", runner.arena().to_string(&value));
            } else {
                println!("{span}: {}", runner.arena().to_string(&value));
            }
            eval_instr_ctx(runner, ctx, &instr.instr, tail)
        }
    })();
    let result = match result {
        Backtrack::Unmatch(errors)
            if matches!(
                instr.node,
                ast::InstrKind::Let(_)
                    | ast::InstrKind::Rule(_)
                    | ast::InstrKind::Result(_)
                    | ast::InstrKind::Return(_)
                    | ast::InstrKind::Debug(_)
            ) =>
        {
            Backtrack::Ok(Flow::Cont(errors))
        }
        result => result,
    };
    result.nest(instr.span.clone(), || {
        ErrorKind::Trace(TraceErrorKind::Instruction { instr: Print::to_string(instr) })
    })
}

fn continuation(span: Span, error: PremErrorKind) -> Backtrack<Flow> {
    Backtrack::Ok(Flow::Cont(vec![Error::new(ErrorKind::Prem(error), span)]))
}

fn condition_iter<Iface: Interface, Exn: Extern>(
    runner: &mut RunnerContext<'_, SlInterp, Iface, Exn>,
    ctx: &Context<'_>,
    iters: &[ast::ExpIter],
    reverse: bool,
    eval: &mut impl FnMut(&mut RunnerContext<'_, SlInterp, Iface, Exn>, &Context<'_>) -> Backtrack<bool>,
) -> Backtrack<bool> {
    let iter = if reverse { iters.split_last() } else { iters.split_first() };
    let Some(((iter, vars), iters_tail)) = iter else {
        return eval(runner, ctx);
    };
    match iter {
        ast::Iter::Opt => {
            let values =
                backtrack_from_result!(ctx.opt_values(runner.arena(), vars), &Span::default());
            let Some(values) = values else {
                return Backtrack::Ok(false);
            };
            let mut ctx_sub = ctx.clone();
            for (var, value) in vars.iter().zip(values) {
                ctx_sub.add_value(Variable::new(var.id.clone(), var.iters.clone()), value);
            }
            // The OCaml optional condition reverses the remaining iteration order
            condition_iter(runner, &ctx_sub, iters_tail, !reverse, eval)
        }
        ast::Iter::List => {
            let rows =
                backtrack_from_result!(ctx.list_values(runner.arena(), vars), &Span::default());
            // Copy handles before the callback can allocate in the arena
            let rows: Vec<_> = rows.into_iter().map(<[Value]>::to_vec).collect();
            let width = rows.first().map_or(0, Vec::len);
            let vars: Vec<_> = vars
                .iter()
                .map(|var| Variable::new(var.id.clone(), var.iters.clone()))
                .collect();
            let mut ctx_sub = ctx.clone();
            for column in 0..width {
                for (var, row) in vars.iter().zip(&rows) {
                    ctx_sub.add_value(var.clone(), row[column]);
                }
                if !backtrack!(condition_iter(runner, &ctx_sub, iters_tail, reverse, eval)) {
                    return Backtrack::Ok(false);
                }
            }
            Backtrack::Ok(true)
        }
    }
}

fn binding_iter<'global, Iface: Interface, Exn: Extern>(
    runner: &mut RunnerContext<'_, SlInterp, Iface, Exn>,
    ctx: Context<'global>,
    iters: &[ast::InstrIter],
    eval: &mut impl FnMut(
        &mut RunnerContext<'_, SlInterp, Iface, Exn>,
        Context<'global>,
    ) -> Backtrack<Context<'global>>,
) -> Backtrack<Context<'global>> {
    let Some((iter, iters_tail)) = iters.split_last() else {
        return eval(runner, ctx);
    };
    match iter.iter {
        ast::Iter::Opt => ctx.yield_opt(
            runner,
            &Span::default(),
            &iter.vars_bound,
            &iter.vars_bind,
            |runner, ctx| binding_iter(runner, ctx, iters_tail, eval),
        ),
        ast::Iter::List => ctx.yield_list(
            runner,
            &Span::default(),
            &iter.vars_bound,
            &iter.vars_bind,
            |runner, ctx| binding_iter(runner, ctx, iters_tail, eval),
        ),
    }
}

fn eval_guard<Iface: Interface, Exn: Extern>(
    runner: &mut RunnerContext<'_, SlInterp, Iface, Exn>,
    ctx: &Context<'_>,
    exp: &ast::Exp,
    value: Value,
    guard: &ast::Guard,
) -> Backtrack<bool> {
    if matches!(guard, ast::Guard::Bool(true)) {
        return Backtrack::from_result(get::bool(runner.arena(), &value), &exp.span);
    }
    let result = (|| match guard {
        ast::Guard::Bool(_) => {
            Backtrack::Ok(!backtrack_from_result!(get::bool(runner.arena(), &value), &exp.span))
        }
        ast::Guard::Cmp(op, _, exp_r) => {
            let value_r = backtrack!(eval_exp(runner, ctx, exp_r));
            ops::compare(runner.arena(), &exp.span, op, value, value_r)
        }
        ast::Guard::Sub(_, check) => ops::check_sub(runner.arena(), ctx, &exp.span, check, value),
        ast::Guard::Match(pattern) => Backtrack::Ok(ops::matches(runner.arena(), pattern, value)),
        ast::Guard::Mem(exp_list) => {
            let value_list = backtrack!(eval_exp(runner, ctx, exp_list));
            ops::contains(runner.arena(), &exp.span, value, value_list)
        }
    })();
    result.nest(exp.span.clone(), || {
        let exp_kind = match guard {
                    ast::Guard::Bool(true) => exp.node.clone(),
                    ast::Guard::Bool(false) => ast::ExpKind::Un(
                        ast::UnOp::Bool(boolean::UnOp::Not),
                        ast::OpTyp::Bool,
                        Box::new(exp.clone()),
                    ),
                    ast::Guard::Cmp(op, typ, exp_r) => {
                        ast::ExpKind::Cmp(*op, *typ, Box::new(exp.clone()), Box::new(exp_r.clone()))
                    }
                    ast::Guard::Sub(typ, check) => ast::ExpKind::Sub(
                        Box::new(exp.clone()),
                        Box::new(typ.clone()),
                        check.clone(),
                    ),
                    ast::Guard::Match(pattern) => {
                        ast::ExpKind::Match(Box::new(exp.clone()), pattern.clone())
                    }
                    ast::Guard::Mem(exp_list) => {
                        ast::ExpKind::Mem(Box::new(exp.clone()), Box::new(exp_list.clone()))
                    }
                };
    let exp_cond = crate::note_phrase!(node: exp_kind, note: std::rc::Rc::new(ast::TypKind::Bool), span: exp.span.clone());

        ErrorKind::Trace(TraceErrorKind::Expression { exp: Print::to_string(&exp_cond) })
    })
}
