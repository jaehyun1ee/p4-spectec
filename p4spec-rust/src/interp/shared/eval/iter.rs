//! Iterated expression evaluation and binding collection

use super::super::{
    backtrack::{Backtrack, ok, unwrap, unwrap_from_result},
    context::IterContext,
    error::Error,
};
use crate::interp::shared::prepare::expr as ast;
use crate::interp::shared::util::iter_vars;
use crate::{
    lang::{common::source::Span, data::value::Value},
    runner::{Extern, Interface, Interpreter, RunnerContext},
};

// = Expression mapping

pub fn map_list<Ctx, Interp, Iface, Ext>(
    runner_ctx: &mut RunnerContext<'_, Interp, Iface, Ext>,
    ctx: &Ctx,
    span: &Span,
    exp_iter: &ast::ExpIter,
    mut eval: impl FnMut(&mut RunnerContext<'_, Interp, Iface, Ext>, &Ctx) -> Backtrack<Value>,
) -> Backtrack<Vec<Value>>
where
    Ctx: IterContext,
    Interp: Interpreter<Iface, Ext, Error = Error>,
    Iface: Interface,
    Ext: Extern,
{
    let vars = &exp_iter.1;
    let vars_outer = iter_vars(ctx, vars, exp_iter.0);
    let values_by_var =
        unwrap_from_result!(ctx.find_list_values_by_var(runner_ctx.arena(), &vars_outer), span);
    // Copy handles before the callback can allocate in the arena
    let values_by_var: Vec<_> = values_by_var.into_iter().map(<[Value]>::to_vec).collect();
    let len = values_by_var.first().map_or(0, Vec::len);
    let mut ctx_sub = ctx.clone();
    let mut values = Vec::with_capacity(len);
    for idx in 0..len {
        for (var, values) in vars.iter().zip(&values_by_var) {
            ctx_sub.add_slot(var.slot, values[idx]);
        }
        values.push(unwrap!(eval(runner_ctx, &ctx_sub)));
    }
    ok!(values)
}

pub fn map_opt<Ctx, Interp, Iface, Ext>(
    runner_ctx: &mut RunnerContext<'_, Interp, Iface, Ext>,
    ctx: &Ctx,
    span: &Span,
    exp_iter: &ast::ExpIter,
    mut eval: impl FnMut(&mut RunnerContext<'_, Interp, Iface, Ext>, &Ctx) -> Backtrack<Value>,
) -> Backtrack<Option<Value>>
where
    Ctx: IterContext,
    Interp: Interpreter<Iface, Ext, Error = Error>,
    Iface: Interface,
    Ext: Extern,
{
    let vars = &exp_iter.1;
    let vars_outer = iter_vars(ctx, vars, exp_iter.0);
    let values =
        unwrap_from_result!(ctx.find_opt_values_by_var(runner_ctx.arena(), &vars_outer), span);
    let Some(values) = values else {
        return ok!(None);
    };
    let mut ctx_sub = ctx.clone();
    for (var, value) in vars.iter().zip(values) {
        ctx_sub.add_slot(var.slot, value);
    }
    ok!(Some(unwrap!(eval(runner_ctx, &ctx_sub))))
}

// = Binding iteration

pub fn yield_list<Ctx, Interp, Iface, Ext>(
    runner_ctx: &mut RunnerContext<'_, Interp, Iface, Ext>,
    mut ctx: Ctx,
    span: &Span,
    prem_iter: &ast::PremIter,
    mut eval: impl FnMut(&mut RunnerContext<'_, Interp, Iface, Ext>, Ctx) -> Backtrack<Ctx>,
) -> Backtrack<Ctx>
where
    Ctx: IterContext,
    Interp: Interpreter<Iface, Ext, Error = Error>,
    Iface: Interface,
    Ext: Extern,
{
    let vars_bound = &prem_iter.vars_bound;
    let vars_bind = &prem_iter.vars_bind;
    let vars_bound_outer = iter_vars(&ctx, vars_bound, prem_iter.iter);
    let vars_bind_outer = iter_vars(&ctx, vars_bind, prem_iter.iter);
    let values_by_var = unwrap_from_result!(
        ctx.find_list_values_by_var(runner_ctx.arena(), &vars_bound_outer),
        span
    );
    let values_by_var: Vec<_> = values_by_var.into_iter().map(<[Value]>::to_vec).collect();
    let len = values_by_var.first().map_or(0, Vec::len);
    let mut ctx_sub = ctx.clone();
    let mut values_bind_by_var = vec![Vec::new(); vars_bind.len()];
    for idx in 0..len {
        for (var, values) in vars_bound.iter().zip(&values_by_var) {
            ctx_sub.add_slot(var.slot, values[idx]);
        }
        // Keep callback writes out of the reusable input context
        let ctx_post = unwrap!(eval(runner_ctx, ctx_sub.clone()));
        unwrap!(ctx_post.collect_values_by_var(vars_bind, &mut values_bind_by_var));
    }
    unwrap!(ctx.bind_list_values_by_var(
        runner_ctx.arena_mut(),
        &vars_bind_outer,
        values_bind_by_var
    ));
    ok!(ctx)
}

pub fn yield_opt<Ctx, Interp, Iface, Ext>(
    runner_ctx: &mut RunnerContext<'_, Interp, Iface, Ext>,
    mut ctx: Ctx,
    span: &Span,
    prem_iter: &ast::PremIter,
    mut eval: impl FnMut(&mut RunnerContext<'_, Interp, Iface, Ext>, Ctx) -> Backtrack<Ctx>,
) -> Backtrack<Ctx>
where
    Ctx: IterContext,
    Interp: Interpreter<Iface, Ext, Error = Error>,
    Iface: Interface,
    Ext: Extern,
{
    let vars_bound = &prem_iter.vars_bound;
    let vars_bind = &prem_iter.vars_bind;
    let vars_bound_outer = iter_vars(&ctx, vars_bound, prem_iter.iter);
    let vars_bind_outer = iter_vars(&ctx, vars_bind, prem_iter.iter);
    let values = unwrap_from_result!(
        ctx.find_opt_values_by_var(runner_ctx.arena(), &vars_bound_outer),
        span
    );
    let mut values_bind_by_var = vec![Vec::new(); vars_bind.len()];
    if let Some(values) = values {
        let mut ctx_sub = ctx.clone();
        for (var, value) in vars_bound.iter().zip(values) {
            ctx_sub.add_slot(var.slot, value);
        }
        let ctx_post = unwrap!(eval(runner_ctx, ctx_sub));
        unwrap!(ctx_post.collect_values_by_var(vars_bind, &mut values_bind_by_var));
    }
    unwrap!(ctx.bind_opt_values_by_var(
        runner_ctx.arena_mut(),
        &vars_bind_outer,
        values_bind_by_var
    ));
    ok!(ctx)
}
