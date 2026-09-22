//! Expression and parameter assignment
//!
//! Re-exports shared expression assignment and binds prepared PL parameters.
//! `assign_params` resolves function arguments in the caller's context.

use crate::{
    interp::{
        pl::context::Context,
        shared::{
            backtrack::{Backtrack, ok, unwrap},
            error::{AssignErrorKind, ErrorKind},
        },
    },
    lang::{common::source::Span, data::value::Value},
    runtime::envs::interp::pl::ast_prepared as ast,
};

pub(super) use crate::interp::shared::eval::assign::{assign_def, assign_exp, assign_exps};

// = Parameter assignment

/// Binds prepared parameter patterns and resolves caller function aliases.
pub(super) fn assign_params<'g>(
    arena: &mut crate::lang::data::value::ValueArena,
    ctx_caller: &Context<'_>,
    mut ctx: Context<'g>,
    params: &[ast::Param],
    values: &[Value],
) -> Backtrack<Context<'g>> {
    // Argument count must match the parameters
    unwrap!(Backtrack::check(
        params.len() == values.len(),
        Span::default(),
        ErrorKind::Assign(AssignErrorKind::ArgumentArityMismatch {
            expected: params.len(),
            actual: values.len()
        })
    ));
    // Bind pairwise, threading the callee context
    for (param, value) in params.iter().zip(values) {
        let result = match &param.node {
            ast::ParamKind::Exp(_, exp) => assign_exp(arena, ctx, exp, *value),
            ast::ParamKind::Def(id, ..) => assign_def(arena, ctx_caller, ctx, id, *value),
        };
        ctx = unwrap!(result);
    }
    ok!(ctx)
}
