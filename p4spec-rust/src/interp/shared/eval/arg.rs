//! Shared argument evaluation

use super::super::context::Environment;
use super::Evaluator;

use crate::{
    lang::{
        common::source::Span,
        data::value::{Value, ValueArena, make},
        il::ast,
    },
    runner::{Extern, Interface, RunnerContext},
};

use super::expr::eval_exp;
use crate::interp::shared::backtrack::{Backtrack, backtrack, backtrack_from_result};

fn eval_arg<Ctx: Environment, Eval: Evaluator<Ctx, Iface, Exn>, Iface: Interface, Exn: Extern>(
    runner: &mut RunnerContext<'_, Eval, Iface, Exn>,
    ctx: &Ctx,
    arg: &ast::Arg,
) -> Backtrack<Value> {
    let result = match &arg.node {
        ast::ArgKind::Exp(exp) => eval_exp(runner, ctx, exp),
        ast::ArgKind::Def(id) => eval_def_arg(runner.arena_mut(), ctx, id, &arg.span),
    };
    Eval::trace_arg(arg, result)
}

pub(crate) fn eval_args<
    Ctx: Environment,
    Eval: Evaluator<Ctx, Iface, Exn>,
    Iface: Interface,
    Exn: Extern,
>(
    runner: &mut RunnerContext<'_, Eval, Iface, Exn>,
    ctx: &Ctx,
    args: &[ast::Arg],
) -> Backtrack<Vec<Value>> {
    let mut values = Vec::with_capacity(args.len());
    for arg in args {
        values.push(backtrack!(eval_arg(runner, ctx, arg)));
    }
    Backtrack::Ok(values)
}

// - Function argument

fn eval_def_arg<Ctx: Environment>(
    arena: &mut ValueArena,
    ctx: &Ctx,
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
