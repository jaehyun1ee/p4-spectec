//! Checks for shallow binding patterns
//!
//! Shallow binders are variables,
//! upcasts of variables or cases,
//! and cases whose arguments are variables under any number of iterations.
//! Table rows require shallow binders without repeated new names.
//! `check_args` validates a complete row before binding lowering.

use crate::lang::{
    common::{ds::map::IdMap, source::Span},
    il::ast,
};

use super::{
    super::{AlgoError, error},
    context::Context,
};

/// Checks for a variable under any number of iterations.
fn is_iterated_id_exp(exp: &ast::Exp) -> bool {
    match &exp.node {
        ast::ExpKind::Id(_) => true,
        ast::ExpKind::Iter(exp, _) => is_iterated_id_exp(exp),
        _ => false,
    }
}

// Expressions

/// Checks whether an expression is a shallow binder.
pub fn check_exp(exp: &ast::Exp) -> bool {
    match &exp.node {
        ast::ExpKind::Id(_) => true,
        ast::ExpKind::UpCast(_, exp) => {
            matches!(&exp.node, ast::ExpKind::Id(_) | ast::ExpKind::Case(_))
        }
        ast::ExpKind::Case(not_exp) => not_exp.args().into_iter().all(is_iterated_id_exp),
        _ => false,
    }
}

// Arguments

/// Checks whether an argument is a shallow binder; function arguments are not.
pub fn check_arg(arg: &ast::Arg) -> bool {
    match &arg.node {
        ast::ArgKind::Exp(exp) => check_exp(exp),
        ast::ArgKind::Def(_) => false,
    }
}

/// Validates shallow table arguments and rejects repeated new binders.
pub fn check_args(ctx: &Context, args: &[ast::Arg]) -> Result<(), AlgoError> {
    // Reject the first invalid shape before traversing its binders
    for arg in args {
        if !check_arg(arg) {
            return Err(error::table::table_binding_shape_invalid(arg));
        }
    }
    // Reject repeated new binders in occurrence order, before renaming
    let mut seen = IdMap::new();
    for arg in args {
        let ast::ArgKind::Exp(exp) = &arg.node else {
            unreachable!("shallow table validation rejects function arguments");
        };
        check_repeated_table_binding(ctx, &mut seen, exp)?;
    }

    Ok(())
}

/// Rejects the first repeated new binder in a validated shallow pattern.
fn check_repeated_table_binding(
    ctx: &Context,
    seen: &mut IdMap<Span>,
    exp: &ast::Exp,
) -> Result<(), AlgoError> {
    match &exp.node {
        ast::ExpKind::Id(id) => {
            // Previously bound names do not introduce table binders
            if ctx.venv.contains_key(id) {
                return Ok(());
            }
            // Keep the first source occurrence even when names sort differently
            if let Some(span_first) = seen.get(id) {
                return Err(error::table::table_binding_repeated(id, span_first));
            }
            seen.insert(id.clone(), id.span.clone());
        }
        ast::ExpKind::UpCast(_, exp) | ast::ExpKind::Iter(exp, _) => {
            // Casts and dimensions retain the underlying binding positions
            check_repeated_table_binding(ctx, seen, exp)?;
        }
        ast::ExpKind::Case(not_exp) => {
            // Visit case arguments from left to right
            for exp in not_exp.args() {
                check_repeated_table_binding(ctx, seen, exp)?;
            }
        }
        ast::ExpKind::Tuple(exps) | ast::ExpKind::List(exps) => {
            // Upcast cases can contain invertible compound arguments
            for exp in exps {
                check_repeated_table_binding(ctx, seen, exp)?;
            }
        }
        ast::ExpKind::Str(exp_fields) => {
            // Preserve field occurrence order beneath an upcast case
            for ast::ExpField { exp, .. } in exp_fields {
                check_repeated_table_binding(ctx, seen, exp)?;
            }
        }
        ast::ExpKind::Opt(Some(exp)) => {
            // An option contributes the binders of its payload
            check_repeated_table_binding(ctx, seen, exp)?;
        }
        ast::ExpKind::Cons(exp_l, exp_r) => {
            // A cons pattern binds its head before its tail
            check_repeated_table_binding(ctx, seen, exp_l)?;
            check_repeated_table_binding(ctx, seen, exp_r)?;
        }
        _ => {
            // Literals bind nothing; collection rejects non-invertible binders
        }
    }
    Ok(())
}
