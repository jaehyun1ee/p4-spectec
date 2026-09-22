//! PL instruction, block, and alternative evaluation
//!
//! `eval_group_block` executes a rule group or function body;
//! `eval_dispatch_block` selects relation groups through routing instructions.
//! `eval_block` isolates bindings; `eval_alternatives` selects conclusions.
//! Expression and assignment adapters remove hints before shared evaluation.

use super::{
    assign::{assign_exp, assign_exps},
    expr::{eval_exp, eval_exps},
};
use crate::{
    interp::{
        pl::{
            PlInterp,
            context::Context,
            flow::{self, Flow},
        },
        shared::{
            backtrack::{Backtrack, err, ok, unmatch, unwrap, unwrap_from_result},
            context::{IterContext, WriteContext},
            error::{ErrorKind, PremErrorKind, TraceErrorKind},
            eval::{self as shared_eval, Invoker},
            prepare::ast as exec,
            util::iterate_vars,
        },
    },
    lang::{
        common::source::Span,
        data::value::{Value, get},
        traits::print::Print,
    },
    runner::{Extern, Interface, RunnerContext},
    runtime::envs::interp::pl::ast_prepared as ast,
};

// = Iterated evaluation

// - Bindings

/// Evaluates bindings from the outermost iteration inward.
fn eval_binding_iters<'g, Iface: Interface, Ext: Extern>(
    runner_ctx: &mut RunnerContext<'_, PlInterp, Iface, Ext>,
    ctx: Context<'g>,
    iters: &[ast::InstrIter],
    eval: &mut impl FnMut(
        &mut RunnerContext<'_, PlInterp, Iface, Ext>,
        Context<'g>,
    ) -> Backtrack<Context<'g>>,
) -> Backtrack<Context<'g>> {
    // The last iteration is outermost; no iterations evaluates the body
    let Some((iter, rest)) = iters.split_last() else {
        return eval(runner_ctx, ctx);
    };
    // Collect bindings while descending into the remaining iterations
    shared_eval::iter::r#yield(runner_ctx, ctx, &Span::default(), iter, |runner_ctx, ctx| {
        eval_binding_iters(runner_ctx, ctx, rest, eval)
    })
}

// - Conditions

/// Tests a condition across the values of each enclosing iteration.
fn eval_condition_iters<Iface: Interface, Ext: Extern>(
    runner_ctx: &mut RunnerContext<'_, PlInterp, Iface, Ext>,
    ctx: &Context<'_>,
    iters: &[ast::ExpIter],
    eval: &mut impl FnMut(&mut RunnerContext<'_, PlInterp, Iface, Ext>, &Context<'_>) -> Backtrack<bool>,
) -> Backtrack<bool> {
    // The last iteration is outermost; no iterations evaluates the body
    let Some((iter, rest)) = iters.split_last() else {
        return eval(runner_ctx, ctx);
    };
    // Read the variables one iteration outward
    let vars_outer = iterate_vars(ctx, &iter.vars, iter.iter);
    match iter.iter {
        exec::Iter::Opt => {
            // An absent option cannot satisfy the condition
            let values = unwrap_from_result!(
                ctx.find_opt_values_by_var(runner_ctx.arena(), &vars_outer),
                &Span::default()
            );
            let Some(values) = values else {
                return ok!(false);
            };
            // Bind this iteration in a local context
            let mut ctx_sub = ctx.clone();
            for (var, value) in iter.vars.iter().zip(values) {
                ctx_sub.add_value(var.slot, value);
            }
            eval_condition_iters(runner_ctx, &ctx_sub, rest, eval)
        }
        exec::Iter::List => {
            // Lists must agree in length before testing corresponding rows
            let values_by_var = unwrap_from_result!(
                ctx.find_list_values_by_var(runner_ctx.arena(), &vars_outer),
                &Span::default()
            );
            let values_by_var = values_by_var
                .into_iter()
                .map(<[Value]>::to_vec)
                .collect::<Vec<_>>();
            let len = values_by_var.first().map_or(0, Vec::len);
            // Bind this iteration in a local context
            let mut ctx_sub = ctx.clone();
            // Every row must satisfy the condition; an empty list does
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

// = Case guards

/// Checks a case guard and binds its pattern only when it matches.
fn eval_guard<'g, Iface: Interface, Ext: Extern>(
    runner_ctx: &mut RunnerContext<'_, PlInterp, Iface, Ext>,
    ctx: Context<'g>,
    value: Value,
    guard: &ast::Guard,
) -> Backtrack<Option<Context<'g>>> {
    // Test the scrutinee before introducing checked bindings
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
    // A failed guard leaves the caller free to try another case
    if !matched {
        return ok!(None);
    }
    // Only checked guards extend the selected case
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

// = Common instructions

/// Evaluates shared control instructions with tier-specific callbacks.
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
    // Nested blocks need stack growth even without a function call
    stacker::maybe_grow(64 * 1024, 1024 * 1024, || match &instr.node.node {
        ast::InstrKind::If(instr) => {
            // Test the condition across all enclosing iterations
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
            // Treat a relation mismatch as false, retaining fatal errors
            let holds = unwrap!(eval_condition_iters(
                runner_ctx,
                &ctx,
                &instr.iter_exps,
                &mut |runner_ctx, ctx| {
                    let exps = instr.not_exp.args();
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
            // Try guards in source order from the enclosing bindings
            let value = unwrap!(eval_exp(runner_ctx, &ctx, &instr.exp));
            for case in &instr.cases {
                if let Some(ctx_arm) =
                    unwrap!(eval_guard(runner_ctx, ctx.clone(), value, &case.guard))
                {
                    // Keep guard bindings inside the selected case
                    let (_, flow) = unwrap!(eval_block(runner_ctx, ctx_arm, &case.block));
                    return ok!((ctx, flow));
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
            // Extend the current block with bindings from each iteration
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
            // Evaluate before printing so expression failures keep their trace
            let value = unwrap!(eval_exp(runner_ctx, &ctx, &instr.exp));
            println!("{}", runner_ctx.arena().to_string(&value));
            ok!((ctx, Flow::Cont(vec![])))
        }
        ast::InstrKind::Destruct(instr) => {
            // Extract fields before mutating the arena during assignment
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
                .map(|(_, exp)| exp)
                .collect::<Vec<_>>();
            let ctx = unwrap!(assign_exps(runner_ctx.arena_mut(), ctx, &exps, &values));
            ok!((ctx, Flow::Cont(vec![])))
        }
        ast::InstrKind::CheckLetSub(instr) => {
            // Check membership before casting and binding the value
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
                    // A successful binding is visible only in the nested block
                    ok!(ctx_bound) => {
                        let (_, flow) = unwrap!(eval_block(runner_ctx, ctx_bound, &instr.block));
                        ok!((ctx, flow))
                    }
                    // A failed binding lets the enclosing block continue
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
            // Check the shape before assigning its pattern
            let value = unwrap!(eval_exp(runner_ctx, &ctx, &instr.exp_r));
            if shared_eval::ops::r#match(runner_ctx.arena(), &instr.pattern, value) {
                // The shorthand binding belongs to the nested block
                let ctx_bound =
                    unwrap!(assign_exp(runner_ctx.arena_mut(), ctx.clone(), &instr.exp_l, value));
                let (_, flow) = unwrap!(eval_block(runner_ctx, ctx_bound, &instr.block));
                ok!((ctx, flow))
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
            // Only a present option enters the nested block
            let value = unwrap!(eval_exp(runner_ctx, &ctx, &instr.exp_r));
            if let Some(value) =
                unwrap_from_result!(get::opt(runner_ctx.arena(), &value), &instr.exp_r.node.span)
            {
                // The shorthand binding belongs to the nested block
                let ctx_bound =
                    unwrap!(assign_exp(runner_ctx.arena_mut(), ctx.clone(), &instr.exp_l, value));
                let (_, flow) = unwrap!(eval_block(runner_ctx, ctx_bound, &instr.block));
                ok!((ctx, flow))
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
        // The callback supplies the group or dispatch semantics
        ast::InstrKind::Tier(instr) => eval_tier(runner_ctx, ctx, &instr.tier),
    })
}

// = Blocks

/// Runs instructions in one local scope, restoring bindings at its boundary.
pub(super) fn eval_block<'g, 'instr, Tier: 'instr, Iface: Interface, Ext: Extern>(
    runner_ctx: &mut RunnerContext<'_, PlInterp, Iface, Ext>,
    ctx: Context<'g>,
    instrs: impl IntoIterator<Item = &'instr ast::Instr<Tier>>,
    mut evaluate: impl FnMut(
        &mut RunnerContext<'_, PlInterp, Iface, Ext>,
        Context<'g>,
        &ast::Instr<Tier>,
    ) -> Backtrack<(Context<'g>, Flow)>,
) -> Backtrack<(Context<'g>, Flow)> {
    // Assignments extend this block, not its enclosing scope
    let mut ctx_local = ctx.clone();
    let mut errors = vec![];
    for instr in instrs {
        let (ctx_post, flow) = unwrap!(evaluate(runner_ctx, ctx_local, instr));
        ctx_local = ctx_post;
        // Retain the deepest failure until an instruction concludes
        match flow {
            // A continuing instruction contributes failure diagnostics
            Flow::Cont(errors_post) => flow::retain_deepest_errors(&mut errors, errors_post),
            // A conclusion leaves the block with its original bindings
            flow => return ok!((ctx, flow)),
        }
    }
    ok!((ctx, Flow::Cont(errors)))
}

// = Alternative selection

/// Chooses between isolated alternatives in either execution mode.
fn eval_alternatives<'g, Tier, Iface: Interface, Ext: Extern>(
    runner_ctx: &mut RunnerContext<'_, PlInterp, Iface, Ext>,
    ctx: Context<'g>,
    blocks: &[ast::Block<Tier>],
    mut evaluate: impl FnMut(
        &mut RunnerContext<'_, PlInterp, Iface, Ext>,
        Context<'g>,
        &ast::Block<Tier>,
    ) -> Backtrack<(Context<'g>, Flow)>,
) -> Backtrack<(Context<'g>, Flow)> {
    let det = runner_ctx.interp().config.det;
    let mut flow = Flow::Cont(vec![]);
    for block in blocks {
        // Every alternative starts from the enclosing bindings
        let flow_post = match evaluate(runner_ctx, ctx.clone(), block) {
            // Local alternative bindings do not escape
            ok!((_, flow)) => flow,
            // Sequential choice retains the most specific mismatch
            unmatch!(errors) if !det => Flow::Cont(errors),
            // Deterministic choice skips mismatching alternatives
            unmatch!(_) => continue,
            // A fatal error aborts alternative selection
            err!(errors) => return err!(errors),
        };
        // Deterministic choice must evaluate every alternative
        if det {
            let span = block
                .first()
                .map_or_else(Span::default, |instr| instr.node.span.clone());
            flow = unwrap!(flow::combine_deterministic(flow, flow_post, &span));
        } else {
            // Sequential choice keeps the first conclusion
            match flow_post {
                // Continue looking after remembering the deepest failure
                Flow::Cont(errors_post) => {
                    let Flow::Cont(errors) = &mut flow else { unreachable!() };
                    flow::retain_deepest_errors(errors, errors_post);
                }
                // The first conclusion selects this alternative
                flow => return ok!((ctx, flow)),
            }
        }
    }
    ok!((ctx, flow))
}

// = Group-body tier

pub(super) fn eval_group_block<'g, Iface: Interface, Ext: Extern>(
    runner_ctx: &mut RunnerContext<'_, PlInterp, Iface, Ext>,
    ctx: Context<'g>,
    block: &ast::GroupBlock,
) -> Backtrack<(Context<'g>, Flow)> {
    eval_block(runner_ctx, ctx, block, eval_group_instr)
}

/// Evaluates group conclusions and bindings, retaining the instruction trace.
pub(super) fn eval_group_instr<'g, Iface: Interface, Ext: Extern>(
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
                // The input hint separates arguments from output patterns
                let args = instr.not_exp.args();
                let (exps_input, exps_output) = unwrap_from_result!(
                    crate::lang::hints::input::split(&instr.input_hint, args),
                    &instr.id.span
                );
                // Invoke the relation at each enclosing iteration
                let ctx = unwrap!(eval_binding_iters(
                    runner_ctx,
                    ctx,
                    &instr.iter_instrs,
                    &mut |runner_ctx, ctx| {
                        let values = unwrap!(eval_exps(runner_ctx, &ctx, &exps_input));
                        let values =
                            unwrap!(PlInterp::invoke_rel(runner_ctx, &ctx, &instr.id, &values));
                        // Extend the local bindings with relation outputs
                        assign_exps(runner_ctx.arena_mut(), ctx, &exps_output, &values)
                    }
                ));
                ok!((ctx, Flow::Cont(vec![])))
            }
            ast::GroupInstr::Backtrack(instr) => {
                eval_alternatives(runner_ctx, ctx, &instr.blocks, eval_group_block)
            }
        },
        &mut eval_group_block,
    )
    // Attach the instruction text to failures
    .nest(instr.node.span.clone(), || {
        ErrorKind::Trace(TraceErrorKind::Evaluation { text: Print::to_string(instr) })
    })
}

// = Dispatch tier

pub(super) fn eval_dispatch_block<'g, Iface: Interface, Ext: Extern>(
    runner_ctx: &mut RunnerContext<'_, PlInterp, Iface, Ext>,
    ctx: Context<'g>,
    block: &ast::DispatchBlock,
) -> Backtrack<(Context<'g>, Flow)> {
    eval_block(runner_ctx, ctx, block, eval_dispatch_instr)
}

/// Routes to groups or alternatives, retaining the instruction trace.
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
                eval_alternatives(runner_ctx, ctx, &instr.blocks, eval_dispatch_block)
            }
        },
        &mut eval_dispatch_block,
    )
    // Attach the instruction text to failures
    .nest(instr.node.span.clone(), || {
        ErrorKind::Trace(TraceErrorKind::Evaluation { text: Print::to_string(instr) })
    })
}
