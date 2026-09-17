//! Iterated expression evaluation and binding collection

use super::super::{
    backtrack::{Backtrack, unwrap, unwrap_from_result},
    context::IterContext,
    error::Error,
};
use crate::{
    lang::{
        common::{Variable, source::Span},
        data::value::Value,
        il::ast,
    },
    runner::{Extern, Interface, Interpreter, RunnerContext},
};

// = Expression mapping

pub fn map_list<Ctx, Interp, Iface, Ext>(
    runner_ctx: &mut RunnerContext<'_, Interp, Iface, Ext>,
    ctx: &Ctx,
    span: &Span,
    vars: &[ast::Var],
    mut eval: impl FnMut(&mut RunnerContext<'_, Interp, Iface, Ext>, &Ctx) -> Backtrack<Value>,
) -> Backtrack<Vec<Value>>
where
    Ctx: IterContext,
    Interp: Interpreter<Iface, Ext, Error = Error>,
    Iface: Interface,
    Ext: Extern,
{
    let values_by_var =
        unwrap_from_result!(ctx.find_list_values_by_var(runner_ctx.arena(), vars), span);
    // Copy handles before the callback can allocate in the arena
    let values_by_var: Vec<_> = values_by_var.into_iter().map(<[Value]>::to_vec).collect();
    let len = values_by_var.first().map_or(0, Vec::len);
    let vars: Vec<_> = vars
        .iter()
        .map(|var| Variable::new(var.id.clone(), var.iters.clone()))
        .collect();
    let mut ctx_sub = ctx.clone();
    let mut values = Vec::with_capacity(len);
    for idx in 0..len {
        for (var, values) in vars.iter().zip(&values_by_var) {
            ctx_sub.add_value(var.clone(), values[idx]);
        }
        values.push(unwrap!(eval(runner_ctx, &ctx_sub)));
    }
    Backtrack::Ok(values)
}

pub fn map_opt<Ctx, Interp, Iface, Ext>(
    runner_ctx: &mut RunnerContext<'_, Interp, Iface, Ext>,
    ctx: &Ctx,
    span: &Span,
    vars: &[ast::Var],
    mut eval: impl FnMut(&mut RunnerContext<'_, Interp, Iface, Ext>, &Ctx) -> Backtrack<Value>,
) -> Backtrack<Option<Value>>
where
    Ctx: IterContext,
    Interp: Interpreter<Iface, Ext, Error = Error>,
    Iface: Interface,
    Ext: Extern,
{
    let values = unwrap_from_result!(ctx.find_opt_values_by_var(runner_ctx.arena(), vars), span);
    let Some(values) = values else {
        return Backtrack::Ok(None);
    };
    let mut ctx_sub = ctx.clone();
    for (var, value) in vars.iter().zip(values) {
        ctx_sub.add_value(Variable::new(var.id.clone(), var.iters.clone()), value);
    }
    Backtrack::Ok(Some(unwrap!(eval(runner_ctx, &ctx_sub))))
}

// = Binding iteration

pub fn yield_list<Ctx, Interp, Iface, Ext>(
    runner_ctx: &mut RunnerContext<'_, Interp, Iface, Ext>,
    mut ctx: Ctx,
    span: &Span,
    vars_bound: &[ast::Var],
    vars_bind: &[ast::Var],
    mut eval: impl FnMut(&mut RunnerContext<'_, Interp, Iface, Ext>, Ctx) -> Backtrack<Ctx>,
) -> Backtrack<Ctx>
where
    Ctx: IterContext,
    Interp: Interpreter<Iface, Ext, Error = Error>,
    Iface: Interface,
    Ext: Extern,
{
    let values_by_var =
        unwrap_from_result!(ctx.find_list_values_by_var(runner_ctx.arena(), vars_bound), span);
    let values_by_var: Vec<_> = values_by_var.into_iter().map(<[Value]>::to_vec).collect();
    let len = values_by_var.first().map_or(0, Vec::len);
    let vars: Vec<_> = vars_bound
        .iter()
        .map(|var| Variable::new(var.id.clone(), var.iters.clone()))
        .collect();
    let mut ctx_sub = ctx.clone();
    let mut values_bind_by_var = vec![Vec::new(); vars_bind.len()];
    for idx in 0..len {
        for (var, values) in vars.iter().zip(&values_by_var) {
            ctx_sub.add_value(var.clone(), values[idx]);
        }
        // Keep callback writes out of the reusable input context
        let ctx_post = unwrap!(eval(runner_ctx, ctx_sub.clone()));
        unwrap!(ctx_post.collect_values_by_var(vars_bind, &mut values_bind_by_var));
    }
    unwrap!(ctx.bind_list_values_by_var(runner_ctx.arena_mut(), vars_bind, values_bind_by_var));
    Backtrack::Ok(ctx)
}

pub fn yield_opt<Ctx, Interp, Iface, Ext>(
    runner_ctx: &mut RunnerContext<'_, Interp, Iface, Ext>,
    mut ctx: Ctx,
    span: &Span,
    vars_bound: &[ast::Var],
    vars_bind: &[ast::Var],
    mut eval: impl FnMut(&mut RunnerContext<'_, Interp, Iface, Ext>, Ctx) -> Backtrack<Ctx>,
) -> Backtrack<Ctx>
where
    Ctx: IterContext,
    Interp: Interpreter<Iface, Ext, Error = Error>,
    Iface: Interface,
    Ext: Extern,
{
    let values =
        unwrap_from_result!(ctx.find_opt_values_by_var(runner_ctx.arena(), vars_bound), span);
    let mut values_bind_by_var = vec![Vec::new(); vars_bind.len()];
    if let Some(values) = values {
        let mut ctx_sub = ctx.clone();
        for (var, value) in vars_bound.iter().zip(values) {
            ctx_sub.add_value(Variable::new(var.id.clone(), var.iters.clone()), value);
        }
        let ctx_post = unwrap!(eval(runner_ctx, ctx_sub));
        unwrap!(ctx_post.collect_values_by_var(vars_bind, &mut values_bind_by_var));
    }
    unwrap!(ctx.bind_opt_values_by_var(runner_ctx.arena_mut(), vars_bind, values_bind_by_var));
    Backtrack::Ok(ctx)
}
