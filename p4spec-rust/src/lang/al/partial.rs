//! Partiality checks for algorithmic-language data
//!
//! A construct is partial when evaluating it can fail to match:
//! calls, rule premises, and hold premises may not apply,
//! and anything containing them inherits that.
//! The structuring pass uses this to decide where backtracking is needed.

use super::ast::*;

/// Checks whether evaluating an expression may fail: does it contain a call.
pub fn is_partial_exp(exp: &Exp) -> bool {
    match &exp.node {
        // Literals and variables always evaluate
        ExpKind::Bool(_) | ExpKind::Num(_) | ExpKind::Text(_) | ExpKind::Id(_) => false,
        // One operand: partial if it is
        ExpKind::Un(_, _, exp)
        | ExpKind::UpCast(_, exp)
        | ExpKind::DownCast(_, exp)
        | ExpKind::Sub(exp, _, _)
        | ExpKind::Match(exp, _)
        | ExpKind::Len(exp)
        | ExpKind::Dot(exp, _)
        | ExpKind::Iter(exp, _) => is_partial_exp(exp),
        // Two operands: partial if either is
        ExpKind::Bin(_, _, exp_l, exp_r)
        | ExpKind::Cmp(_, _, exp_l, exp_r)
        | ExpKind::Cons(exp_l, exp_r)
        | ExpKind::Cat(exp_l, exp_r)
        | ExpKind::Mem(exp_l, exp_r)
        | ExpKind::Idx(exp_l, exp_r) => is_partial_exp(exp_l) || is_partial_exp(exp_r),
        // Aggregates: partial if any component is
        ExpKind::Tuple(exps) | ExpKind::List(exps) => exps.iter().any(is_partial_exp),
        // Notation arguments likewise
        ExpKind::Case(not_exp) => not_exp.args().into_iter().any(is_partial_exp),
        // Struct fields likewise
        ExpKind::Str(fields) => fields
            .iter()
            .any(|ExpField { exp, .. }| is_partial_exp(exp)),
        // An absent option always evaluates
        ExpKind::Opt(exp) => exp.as_deref().is_some_and(is_partial_exp),
        // Three operands
        ExpKind::Slice(exp_base, exp_idx, exp_len) => {
            is_partial_exp(exp_base) || is_partial_exp(exp_idx) || is_partial_exp(exp_len)
        }
        // Base, path, and replacement
        ExpKind::Upd(exp_base, path, exp_field) => {
            is_partial_exp(exp_base) || is_partial_path(path) || is_partial_exp(exp_field)
        }
        // A call may hit no matching clause
        ExpKind::Call(..) => true,
    }
}

/// Checks whether a path may fail during evaluation.
pub fn is_partial_path(path: &Path) -> bool {
    match &path.node {
        // The root is the value itself
        PathKind::Root => false,
        // Prefix or index
        PathKind::Idx(path, exp_idx) => is_partial_path(path) || is_partial_exp(exp_idx),
        // Prefix, index, or length
        PathKind::Slice(path, exp_idx, exp_len) => {
            is_partial_path(path) || is_partial_exp(exp_idx) || is_partial_exp(exp_len)
        }
        // Prefix only; a field access cannot fail
        PathKind::Dot(path, _) => is_partial_path(path),
    }
}

/// Checks whether a premise may fail during evaluation.
pub fn is_partial_prem(prem: &Prem) -> bool {
    match &prem.node {
        // Conditions and relation calls may not hold
        PremKind::Rule(_) | PremKind::If(_) | PremKind::IfHold(_) | PremKind::IfNotHold(_) => true,
        // A binding fails only if its right side does
        PremKind::Let(prem) => is_partial_exp(&prem.exp_r),
        // As its body
        PremKind::Iter(prem) => is_partial_prem(&prem.prem),
        // As the printed expression
        PremKind::Debug(prem) => is_partial_exp(&prem.exp),
    }
}
