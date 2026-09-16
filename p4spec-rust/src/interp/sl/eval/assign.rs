//! Expression and parameter assignment

use super::super::context::Context;
use crate::interp::shared::error::{AssignErrorKind, ErrorKind};
use crate::{
    interp::shared::backtrack::{Backtrack, backtrack, backtrack_from_result},
    lang::{
        data::value::{Value, ValueArena, ValueKind},
        sl::ast,
    },
};
use std::rc::Rc;

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
        ast::ParamKind::Def(id, ..) => assign_def_param(arena, ctx_caller, ctx, id, value),
    }
}

pub(in crate::interp::sl) fn assign_params<'global>(
    arena: &mut ValueArena,
    ctx_caller: &Context<'_>,
    mut ctx: Context<'global>,
    params: &[ast::Param],
    values: &[Value],
) -> Backtrack<Context<'global>> {
    backtrack!(Backtrack::check(
        params.len() == values.len(),
        crate::lang::common::source::Span::default(),
        ErrorKind::Assign(AssignErrorKind::ArgumentArityMismatch {
            expected: params.len(),
            actual: values.len()
        })
    ));
    for (param, value) in params.iter().zip(values) {
        ctx = backtrack!(assign_param(arena, ctx_caller, ctx, param, *value));
    }
    Backtrack::Ok(ctx)
}

// - Function parameter

fn assign_def_param<'global>(
    arena: &ValueArena,
    ctx_caller: &Context<'_>,
    mut ctx: Context<'global>,
    id: &ast::Id,
    value: Value,
) -> Backtrack<Context<'global>> {
    let ValueKind::Func(id_func) = arena.kind(&value) else {
        return Backtrack::err(
            id.span.clone(),
            ErrorKind::Assign(AssignErrorKind::DefinitionMismatch {
                value: arena.to_string(&value),
                def: id.node.clone(),
            }),
        );
    };
    let (_, func) = backtrack_from_result!(ctx_caller.find_func(id_func), &id_func.span);
    backtrack_from_result!(ctx.add_func(id.clone(), Rc::clone(func)), &id.span);
    Backtrack::Ok(ctx)
}
