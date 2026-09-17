//! Iterated expression evaluation and binding collection

use super::super::{AlInterp, context::Context};
use crate::{
    interp::shared::backtrack::{Backtrack, backtrack, backtrack_from_result},
    lang::{
        al::ast,
        common::{Variable, source::Span},
        data::value::Value,
    },
    runner::{Extern, Interface, RunnerContext},
};

// = Expression mapping

pub fn map_list<'global, Iface: Interface, Exn: Extern>(
    runner: &mut RunnerContext<'_, AlInterp, Iface, Exn>,
    ctx: &Context<'global>,
    span: &Span,
    vars: &[ast::Var],
    mut eval: impl FnMut(
        &mut RunnerContext<'_, AlInterp, Iface, Exn>,
        &Context<'global>,
    ) -> Backtrack<Value>,
) -> Backtrack<Vec<Value>> {
    let rows = backtrack_from_result!(ctx.list_values(runner.arena(), vars), span);
    // Copy handles before the callback can allocate in the arena
    let rows: Vec<_> = rows.into_iter().map(<[Value]>::to_vec).collect();
    let width = rows.first().map_or(0, Vec::len);
    let vars: Vec<_> = vars
        .iter()
        .map(|var| Variable::new(var.id.clone(), var.iters.clone()))
        .collect();
    let mut ctx_sub = ctx.clone();
    let mut values = Vec::with_capacity(width);
    for column in 0..width {
        for (var, row) in vars.iter().zip(&rows) {
            ctx_sub.add_value(var.clone(), row[column]);
        }
        values.push(backtrack!(eval(runner, &ctx_sub)));
    }
    Backtrack::Ok(values)
}

pub fn map_opt<'global, Iface: Interface, Exn: Extern>(
    runner: &mut RunnerContext<'_, AlInterp, Iface, Exn>,
    ctx: &Context<'global>,
    span: &Span,
    vars: &[ast::Var],
    mut eval: impl FnMut(
        &mut RunnerContext<'_, AlInterp, Iface, Exn>,
        &Context<'global>,
    ) -> Backtrack<Value>,
) -> Backtrack<Option<Value>> {
    let values = backtrack_from_result!(ctx.opt_values(runner.arena(), vars), span);
    let Some(values) = values else {
        return Backtrack::Ok(None);
    };
    let mut ctx_sub = ctx.clone();
    for (var, value) in vars.iter().zip(values) {
        ctx_sub.add_value(Variable::new(var.id.clone(), var.iters.clone()), value);
    }
    Backtrack::Ok(Some(backtrack!(eval(runner, &ctx_sub))))
}

// = Binding iteration

pub fn yield_list<'global, Iface: Interface, Exn: Extern>(
    runner: &mut RunnerContext<'_, AlInterp, Iface, Exn>,
    mut ctx: Context<'global>,
    span: &Span,
    vars_bound: &[ast::Var],
    vars_bind: &[ast::Var],
    mut eval: impl FnMut(
        &mut RunnerContext<'_, AlInterp, Iface, Exn>,
        Context<'global>,
    ) -> Backtrack<Context<'global>>,
) -> Backtrack<Context<'global>> {
    let rows = backtrack_from_result!(ctx.list_values(runner.arena(), vars_bound), span);
    let rows: Vec<_> = rows.into_iter().map(<[Value]>::to_vec).collect();
    let width = rows.first().map_or(0, Vec::len);
    let vars: Vec<_> = vars_bound
        .iter()
        .map(|var| Variable::new(var.id.clone(), var.iters.clone()))
        .collect();
    let mut ctx_sub = ctx.clone();
    let mut values_bind = vec![Vec::new(); vars_bind.len()];
    for column in 0..width {
        for (var, row) in vars.iter().zip(&rows) {
            ctx_sub.add_value(var.clone(), row[column]);
        }
        // Keep callback writes out of the reusable input context
        let ctx_post = backtrack!(eval(runner, ctx_sub.clone()));
        backtrack!(ctx_post.collect_bindings(vars_bind, &mut values_bind));
    }
    backtrack!(ctx.bind_iter(runner.arena_mut(), vars_bind, ast::Iter::List, values_bind));
    Backtrack::Ok(ctx)
}

pub fn yield_opt<'global, Iface: Interface, Exn: Extern>(
    runner: &mut RunnerContext<'_, AlInterp, Iface, Exn>,
    mut ctx: Context<'global>,
    span: &Span,
    vars_bound: &[ast::Var],
    vars_bind: &[ast::Var],
    mut eval: impl FnMut(
        &mut RunnerContext<'_, AlInterp, Iface, Exn>,
        Context<'global>,
    ) -> Backtrack<Context<'global>>,
) -> Backtrack<Context<'global>> {
    let values = backtrack_from_result!(ctx.opt_values(runner.arena(), vars_bound), span);
    let mut values_bind = vec![Vec::new(); vars_bind.len()];
    if let Some(values) = values {
        let mut ctx_sub = ctx.clone();
        for (var, value) in vars_bound.iter().zip(values) {
            ctx_sub.add_value(Variable::new(var.id.clone(), var.iters.clone()), value);
        }
        let ctx_post = backtrack!(eval(runner, ctx_sub));
        backtrack!(ctx_post.collect_bindings(vars_bind, &mut values_bind));
    }
    backtrack!(ctx.bind_iter(runner.arena_mut(), vars_bind, ast::Iter::Opt, values_bind));
    Backtrack::Ok(ctx)
}
