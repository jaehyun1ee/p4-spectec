//! Checks for shallow binding patterns.
//!
//! Shallow binders are variables, upcasts of variables or cases, and cases whose arguments
//! are variables under any number of iterations.

use crate::lang::il::ast;

fn is_iterated_var(exp: &ast::Exp) -> bool {
    match &exp.node {
        ast::ExpKind::Var(_) => true,
        ast::ExpKind::Iter(exp, _) => is_iterated_var(exp),
        _ => false,
    }
}

// Expressions

pub fn check_exp(exp: &ast::Exp) -> bool {
    match &exp.node {
        ast::ExpKind::Var(_) => true,
        ast::ExpKind::UpCast(_, exp) => {
            matches!(&exp.node, ast::ExpKind::Var(_) | ast::ExpKind::Case(_))
        }
        ast::ExpKind::Case(not_exp) => not_exp.args().into_iter().all(is_iterated_var),
        _ => false,
    }
}

// Arguments

pub fn check_arg(arg: &ast::Arg) -> bool {
    match &arg.node {
        ast::ArgKind::Exp(exp) => check_exp(exp),
        ast::ArgKind::Def(_) => false,
    }
}

pub fn check_args(args: &[ast::Arg]) -> bool {
    args.iter().all(check_arg)
}
