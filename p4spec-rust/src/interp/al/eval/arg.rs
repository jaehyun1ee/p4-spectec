//! AL argument evaluation

use std::rc::Rc;

use crate::{
    lang::{
        al::ast,
        common::source::Span,
        data::value::{Value, make},
    },
    runner::{Extern, Interface, RunnerContext},
};

use super::super::{
    Al,
    backtrack::{Backtrack, back},
    context::Context,
};
use super::expr::eval_exp;

fn eval_arg<I: Interface, E: Extern>(
    runner: &mut RunnerContext<'_, Al, I, E>,
    ctx: &Context<'_>,
    arg: &ast::Arg,
) -> Backtrack<Rc<Value>> {
    match &arg.node {
        ast::ArgKind::Exp(exp) => eval_exp(runner, ctx, exp),
        ast::ArgKind::Def(id) => eval_def_arg(ctx, id, &arg.span),
    }
}

pub(super) fn eval_args<I: Interface, E: Extern>(
    runner: &mut RunnerContext<'_, Al, I, E>,
    ctx: &Context<'_>,
    args: &[ast::Arg],
) -> Backtrack<Vec<Rc<Value>>> {
    let mut values = Vec::with_capacity(args.len());
    for arg in args {
        values.push(back!(eval_arg(runner, ctx, arg)));
    }
    Backtrack::Ok(values)
}

// - Function argument

fn eval_def_arg(ctx: &Context<'_>, id: &ast::Id, span: &Span) -> Backtrack<Rc<Value>> {
    let typ_func = back!(Backtrack::from_result(ctx.find_func_typ(id), span));
    let value = make::func(
        id.clone(),
        typ_func.tparams,
        typ_func.typs_params,
        *typ_func.typ_ret,
        Span::default(),
    );
    Backtrack::Ok(value)
}
