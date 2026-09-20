//! Expression and parameter assignment

use super::super::context::Context;
use crate::interp::shared::error::{AssignErrorKind, ErrorKind};
use crate::runtime::envs::interp::sl::ast_prepared as ast;
use crate::{
    interp::shared::backtrack::{Backtrack, ok, unwrap},
    lang::data::value::{Value, ValueArena},
};

pub use crate::interp::shared::eval::assign::*;

// = Parameter assignment

fn assign_param<'global>(
    arena: &mut ValueArena,
    ctx_caller: &Context<'_>,
    ctx: Context<'global>,
    param: &ast::Param,
    value: Value,
) -> Backtrack<Context<'global>> {
    match &param.node {
        ast::ParamKind::Exp(_, exp) => assign_exp(arena, ctx, exp, value),
        ast::ParamKind::Def(id, ..) => assign_def(arena, ctx_caller, ctx, id, value),
    }
}

pub(in crate::interp::sl) fn assign_params<'global>(
    arena: &mut ValueArena,
    ctx_caller: &Context<'_>,
    mut ctx: Context<'global>,
    params: &[ast::Param],
    values: &[Value],
) -> Backtrack<Context<'global>> {
    unwrap!(Backtrack::check(
        params.len() == values.len(),
        crate::lang::common::source::Span::default(),
        ErrorKind::Assign(AssignErrorKind::ArgumentArityMismatch {
            expected: params.len(),
            actual: values.len()
        })
    ));
    for (param, value) in params.iter().zip(values) {
        ctx = unwrap!(assign_param(arena, ctx_caller, ctx, param, *value));
    }
    ok!(ctx)
}
