//! AL argument evaluation

use crate::{
    lang::{
        al::ast,
        common::source::Span,
        data::value::{Value, ValueArena, make},
    },
    runner::{Extern, Interface, RunnerContext},
};

use super::super::{
    Al,
    backtrack::{Backtrack, backtrack, backtrack_from_result},
    context::Context,
};
use super::expr::eval_exp;

fn eval_arg<I: Interface, E: Extern>(
    runner: &mut RunnerContext<'_, Al, I, E>,
    ctx: &Context<'_>,
    arg: &ast::Arg,
) -> Backtrack<Value> {
    match &arg.node {
        ast::ArgKind::Exp(exp) => eval_exp(runner, ctx, exp),
        ast::ArgKind::Def(id) => eval_def_arg(runner.arena_mut(), ctx, id, &arg.span),
    }
}

pub(super) fn eval_args<I: Interface, E: Extern>(
    runner: &mut RunnerContext<'_, Al, I, E>,
    ctx: &Context<'_>,
    args: &[ast::Arg],
) -> Backtrack<Vec<Value>> {
    let mut values = Vec::with_capacity(args.len());
    for arg in args {
        values.push(backtrack!(eval_arg(runner, ctx, arg)));
    }
    Backtrack::Ok(values)
}

// - Function argument

fn eval_def_arg(
    arena: &mut ValueArena,
    ctx: &Context<'_>,
    id: &ast::Id,
    span: &Span,
) -> Backtrack<Value> {
    let typ_func = backtrack_from_result!(ctx.find_func_typ(id), span);
    let value = backtrack_from_result!(
        make::func(
            arena,
            id.clone(),
            typ_func.tparams,
            typ_func.typs_params,
            *typ_func.typ_ret,
            Span::default()
        ),
        span
    );
    Backtrack::Ok(value)
}
