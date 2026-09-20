//! Checks for shallow binding patterns
//!
//! Shallow binders are variables,
//! upcasts of variables or cases,
//! and cases whose arguments are variables under any number of iterations.
//! Table rows require shallow binders (`analyze::analyze_table_row`).

use crate::lang::il::ast;

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

pub fn check_args(args: &[ast::Arg]) -> bool {
    args.iter().all(check_arg)
}
