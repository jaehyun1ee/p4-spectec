//! Shared argument evaluation

use super::super::context::ReadContext;
use super::Invoker;
use crate::interp::shared::prepare::ast;

use crate::{
    lang::{
        common::source::Span,
        data::value::{Value, ValueArena, make},
        traits::print::Print,
    },
    runner::{Extern, Interface, RunnerContext},
};

use super::expr::eval_exp;
use crate::interp::shared::{
    backtrack::{Backtrack, ok, unwrap, unwrap_from_result},
    error::{ErrorKind, TraceErrorKind},
};

fn eval_arg<'global, Interp: Invoker<Iface, Ext>, Iface: Interface, Ext: Extern>(
    runner_ctx: &mut RunnerContext<'_, Interp, Iface, Ext>,
    ctx: &Interp::Context<'global>,
    arg: &ast::Arg,
) -> Backtrack<Value> {
    let result = match &arg.node {
        ast::ArgKind::Exp(exp) => eval_exp(runner_ctx, ctx, exp),
        ast::ArgKind::Def(id) => eval_def_arg(runner_ctx.arena_mut(), ctx, id, &arg.span),
    };
    result.nest(arg.span.clone(), || {
        ErrorKind::Trace(TraceErrorKind::Evaluation { text: Print::to_string(arg) })
    })
}

pub(crate) fn eval_args<'global, Interp: Invoker<Iface, Ext>, Iface: Interface, Ext: Extern>(
    runner_ctx: &mut RunnerContext<'_, Interp, Iface, Ext>,
    ctx: &Interp::Context<'global>,
    args: &[ast::Arg],
) -> Backtrack<Vec<Value>> {
    let mut values = Vec::with_capacity(args.len());
    for arg in args {
        values.push(unwrap!(eval_arg(runner_ctx, ctx, arg)));
    }
    ok!(values)
}

// - Function argument

fn eval_def_arg(
    arena: &mut ValueArena,
    ctx: &impl ReadContext,
    id: &ast::Id,
    span: &Span,
) -> Backtrack<Value> {
    let typ_func = unwrap_from_result!(ctx.find_func_typ(id), span);
    let value = unwrap_from_result!(
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
    ok!(value)
}
