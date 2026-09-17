//! Structured branch selection and explicit tail-call flow

use super::super::{
    SlInterp,
    context::{Context, Scope},
    flow::Flow,
};
use super::{
    assign,
    call::{invoke_func, invoke_rel},
    expr::{self, eval_exp, eval_exps},
};
use crate::interp::shared::eval::ops;
use crate::{
    interp::shared::{
        backtrack::{Backtrack, backtrack, backtrack_from_result},
        error::{Error, ErrorKind, PremErrorKind, TraceErrorKind},
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

// = Block evaluation

pub fn eval_block<Iface: Interface, Exn: Extern>(
    runner: &mut RunnerContext<'_, SlInterp, Iface, Exn>,
    ctx: Cow<'_, Context<'_>>,
    block: &[ast::Instr],
    tail: bool,
) -> Backtrack<Flow> {
    if runner.interp().config.det {
        eval_block_deterministic(runner, ctx.as_ref(), block, tail)
    } else {
        eval_block_sequential(runner, ctx, block.iter(), tail)
    }
}

fn eval_block_deterministic<Iface: Interface, Exn: Extern>(
    runner: &mut RunnerContext<'_, SlInterp, Iface, Exn>,
    ctx: &Context<'_>,
    block: &[ast::Instr],
    tail: bool,
) -> Backtrack<Flow> {
    let mut flow = Flow::Cont(vec![]);
    for instr in block {
        let flow_post = match eval_instr(runner, Cow::Borrowed(ctx), instr, tail) {
            Backtrack::Unmatch(_) => continue,
            result => backtrack!(result),
        };
        flow = backtrack!(flow.merge(flow_post, &instr.span));
    }
    Backtrack::Ok(flow)
}

pub(crate) fn eval_block_sequential<'instr, Iface: Interface, Exn: Extern>(
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
        match backtrack!(eval_instr(runner, Cow::Borrowed(ctx.as_ref()), instr, false)) {
            Flow::Cont(errors_post) => retain_deepest_errors(&mut errors, errors_post),
            flow => return Backtrack::Ok(flow),
        }
    }
    match backtrack!(eval_instr(runner, ctx, instr_last, tail)) {
        Flow::Cont(errors_post) => {
            retain_deepest_errors(&mut errors, errors_post);
            Backtrack::Ok(Flow::Cont(errors))
        }
        flow => Backtrack::Ok(flow),
    }
}

fn retain_deepest_errors(errors: &mut Vec<Error>, errors_post: Vec<Error>) {
    if errors_post.iter().map(Error::depth).max().unwrap_or(0)
        >= errors.iter().map(Error::depth).max().unwrap_or(0)
    {
        *errors = errors_post;
    }
}

pub(crate) fn eval_block_with_else<Iface: Interface, Exn: Extern>(
    runner: &mut RunnerContext<'_, SlInterp, Iface, Exn>,
    ctx: Context<'_>,
    block: &[ast::Instr],
    block_else: Option<&[ast::Instr]>,
) -> Backtrack<Flow> {
    let Some(block_else) = block_else else {
        return eval_block(runner, Cow::Owned(ctx), block, true);
    };
    let flow = backtrack!(eval_block(runner, Cow::Borrowed(&ctx), block, false));
    if matches!(flow, Flow::Cont(_)) {
        eval_block(runner, Cow::Owned(ctx), block_else, true)
    } else {
        Backtrack::Ok(flow)
    }
}

// = Instruction evaluation

pub fn eval_instr<Iface: Interface, Exn: Extern>(
    runner: &mut RunnerContext<'_, SlInterp, Iface, Exn>,
    ctx: Cow<'_, Context<'_>>,
    instr: &ast::Instr,
    tail: bool,
) -> Backtrack<Flow> {
    stacker::maybe_grow(64 * 1024, 1024 * 1024, || {
        let result = match &instr.node {
            ast::InstrKind::If(instr) => eval_if_instr(runner, ctx, instr, tail),
            ast::InstrKind::Hold(instr) => eval_hold_instr(runner, ctx, instr, tail),
            ast::InstrKind::Case(instr) => eval_case_instr(runner, ctx, instr, tail),
            ast::InstrKind::Group(instr) => eval_group_instr(runner, ctx, instr, tail),
            ast::InstrKind::Let(instr) => {
                Flow::cont_from_unmatch(eval_let_instr(runner, ctx, instr, tail))
            }
            ast::InstrKind::Rule(instr) => {
                Flow::cont_from_unmatch(eval_rule_instr(runner, ctx, instr, tail))
            }
            ast::InstrKind::Result(instr) => {
                Flow::cont_from_unmatch(eval_result_instr(runner, ctx, instr))
            }
            ast::InstrKind::Return(instr) => {
                Flow::cont_from_unmatch(eval_return_instr(runner, ctx, instr, tail))
            }
            ast::InstrKind::Debug(instr) => {
                Flow::cont_from_unmatch(eval_debug_instr(runner, ctx, instr, tail))
            }
        };
        result.nest(instr.span.clone(), || {
            ErrorKind::Trace(TraceErrorKind::Instruction { instr: Print::to_string(instr) })
        })
    })
}

// - If instruction

fn eval_if_instr<Iface: Interface, Exn: Extern>(
    runner: &mut RunnerContext<'_, SlInterp, Iface, Exn>,
    ctx: Cow<'_, Context<'_>>,
    instr: &ast::IfInstr,
    tail: bool,
) -> Backtrack<Flow> {
    let cond = backtrack!(eval_cond_iter(
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
        eval_block(runner, ctx, &instr.block, tail)
    } else {
        Backtrack::Ok(Flow::cont(
            instr.exp.span.clone(),
            PremErrorKind::ConditionNotMet { exp: Print::to_string(&instr.exp) },
        ))
    }
}

// - Hold instruction

fn eval_hold_instr<Iface: Interface, Exn: Extern>(
    runner: &mut RunnerContext<'_, SlInterp, Iface, Exn>,
    ctx: Cow<'_, Context<'_>>,
    instr: &ast::HoldInstr,
    tail: bool,
) -> Backtrack<Flow> {
    let cond = backtrack!(eval_cond_iter(
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
            eval_block(runner, ctx, if cond { block_hold } else { block_not }, tail)
        }
        ast::HoldCase::Hold(block, _) if cond => eval_block(runner, ctx, block, tail),
        ast::HoldCase::NotHold(block, _) if !cond => eval_block(runner, ctx, block, tail),
        ast::HoldCase::Hold(..) => Backtrack::Ok(Flow::cont(
            instr.id.span.clone(),
            PremErrorKind::HoldConditionNotMet { relation: instr.id.node.clone() },
        )),
        ast::HoldCase::NotHold(..) => Backtrack::Ok(Flow::cont(
            instr.id.span.clone(),
            PremErrorKind::NotHoldConditionNotMet { relation: instr.id.node.clone() },
        )),
    }
}

// - Case instruction

fn eval_case_instr<Iface: Interface, Exn: Extern>(
    runner: &mut RunnerContext<'_, SlInterp, Iface, Exn>,
    ctx: Cow<'_, Context<'_>>,
    instr: &ast::CaseInstr,
    tail: bool,
) -> Backtrack<Flow> {
    let value = backtrack!(eval_exp(runner, ctx.as_ref(), &instr.exp));
    let id = crate::phrase!(node: "~case".to_owned(), span: Span::default());
    let mut ctx_guard = ctx.as_ref().clone();
    ctx_guard.add_value(Variable::new(id.clone(), vec![]), value);
    let exp = crate::note_phrase!(node: ast::ExpKind::Var(id), note: instr.exp.note.clone(), span: instr.exp.span.clone());
    for case in &instr.cases {
        if backtrack!(eval_guard(runner, &ctx_guard, &exp, value, &case.guard)) {
            drop(ctx_guard);
            return eval_block(runner, ctx, &case.block, tail);
        }
    }
    Backtrack::Ok(Flow::cont(
        instr.exp.span.clone(),
        PremErrorKind::ConditionNotMet { exp: format!("case {}", Print::to_string(&instr.exp)) },
    ))
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
            ast::Guard::Sub(typ, check) => {
                ast::ExpKind::Sub(Box::new(exp.clone()), Box::new(typ.clone()), check.clone())
            }
            ast::Guard::Match(pattern) => {
                ast::ExpKind::Match(Box::new(exp.clone()), pattern.clone())
            }
            ast::Guard::Mem(exp_list) => {
                ast::ExpKind::Mem(Box::new(exp.clone()), Box::new(exp_list.clone()))
            }
        };
        let exp_cond = crate::note_phrase!(
            node: exp_kind,
            note: std::rc::Rc::new(ast::TypKind::Bool),
            span: exp.span.clone()
        );
        ErrorKind::Trace(TraceErrorKind::Expression { exp: Print::to_string(&exp_cond) })
    })
}

// - Group instruction

fn eval_group_instr<Iface: Interface, Exn: Extern>(
    runner: &mut RunnerContext<'_, SlInterp, Iface, Exn>,
    ctx: Cow<'_, Context<'_>>,
    instr: &ast::GroupInstr,
    tail: bool,
) -> Backtrack<Flow> {
    eval_block(runner, ctx, &instr.block, tail)
}

// - Let instruction

fn eval_let_instr<Iface: Interface, Exn: Extern>(
    runner: &mut RunnerContext<'_, SlInterp, Iface, Exn>,
    ctx: Cow<'_, Context<'_>>,
    instr: &ast::LetInstr,
    tail: bool,
) -> Backtrack<Flow> {
    let ctx = backtrack!(eval_instr_iter(
        runner,
        ctx.into_owned(),
        &instr.iter_instrs,
        &mut |runner, ctx| {
            let value = backtrack!(eval_exp(runner, &ctx, &instr.exp_r));
            assign::assign_exp(runner.arena_mut(), ctx, &instr.exp_l, value)
        }
    ));
    eval_block(runner, Cow::Owned(ctx), &instr.block, tail)
}

// - Rule instruction

fn eval_rule_instr<Iface: Interface, Exn: Extern>(
    runner: &mut RunnerContext<'_, SlInterp, Iface, Exn>,
    ctx: Cow<'_, Context<'_>>,
    instr: &ast::RuleInstr,
    tail: bool,
) -> Backtrack<Flow> {
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
    let ctx = backtrack!(eval_instr_iter(
        runner,
        ctx.into_owned(),
        &instr.iter_instrs,
        &mut |runner, ctx| {
            let values = backtrack!(eval_exps(runner, &ctx, &exps_input));
            let values = backtrack!(invoke_rel(runner, &ctx, &instr.id, &values));
            assign::assign_exps(runner.arena_mut(), ctx, &exps_output, &values)
        }
    ));
    eval_block(runner, Cow::Owned(ctx), &instr.block, tail)
}

// - Result instruction

fn eval_result_instr<Iface: Interface, Exn: Extern>(
    runner: &mut RunnerContext<'_, SlInterp, Iface, Exn>,
    ctx: Cow<'_, Context<'_>>,
    instr: &ast::ResultInstr,
) -> Backtrack<Flow> {
    Backtrack::Ok(Flow::Result(backtrack!(eval_exps(runner, ctx.as_ref(), &instr.exps))))
}

// - Return instruction

fn eval_return_instr<Iface: Interface, Exn: Extern>(
    runner: &mut RunnerContext<'_, SlInterp, Iface, Exn>,
    ctx: Cow<'_, Context<'_>>,
    instr: &ast::ReturnInstr,
    tail: bool,
) -> Backtrack<Flow> {
    if tail && let ast::ExpKind::Call(id, targs, args) = &instr.exp.node {
        let targs = backtrack_from_result!(expr::resolve_targs(ctx.as_ref(), targs), &id.span);
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

// - Debug instruction

fn eval_debug_instr<Iface: Interface, Exn: Extern>(
    runner: &mut RunnerContext<'_, SlInterp, Iface, Exn>,
    ctx: Cow<'_, Context<'_>>,
    instr: &ast::DebugInstr,
    tail: bool,
) -> Backtrack<Flow> {
    let value = backtrack!(eval_exp(runner, ctx.as_ref(), &instr.exp));
    println!("{}: {}", instr.exp.span, Print::to_string(&instr.exp));
    let span = runner.arena().span(&value).to_string();
    if span.is_empty() {
        println!("{}", runner.arena().to_string(&value));
    } else {
        println!("{span}: {}", runner.arena().to_string(&value));
    }
    eval_instr(runner, ctx, &instr.instr, tail)
}

// = Iteration

// - Condition iteration

fn eval_cond_iter<Iface: Interface, Exn: Extern>(
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
            eval_cond_iter(runner, &ctx_sub, iters_tail, !reverse, eval)
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
                if !backtrack!(eval_cond_iter(runner, &ctx_sub, iters_tail, reverse, eval)) {
                    return Backtrack::Ok(false);
                }
            }
            Backtrack::Ok(true)
        }
    }
}

// - Binding iteration

fn eval_instr_iter<'global, Iface: Interface, Exn: Extern>(
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
        ast::Iter::Opt => super::iter::yield_opt(
            runner,
            ctx,
            &Span::default(),
            &iter.vars_bound,
            &iter.vars_bind,
            |runner, ctx| eval_instr_iter(runner, ctx, iters_tail, eval),
        ),
        ast::Iter::List => super::iter::yield_list(
            runner,
            ctx,
            &Span::default(),
            &iter.vars_bound,
            &iter.vars_bind,
            |runner, ctx| eval_instr_iter(runner, ctx, iters_tail, eval),
        ),
    }
}
