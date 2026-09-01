//! Partiality checks for algorithmic-language data

use super::ast::*;

/// A construct is partial when its evaluation can fail because it invokes a
/// relation or function that may not match
pub fn is_partial_exp(exp: &Exp) -> bool {
    match &exp.node {
        ExpKind::Bool(_) | ExpKind::Num(_) | ExpKind::Text(_) | ExpKind::Var(_) => false,
        ExpKind::Un(_, _, exp)
        | ExpKind::UpCast(_, exp)
        | ExpKind::DownCast(_, exp)
        | ExpKind::Sub(exp, _, _)
        | ExpKind::Match(exp, _)
        | ExpKind::Len(exp)
        | ExpKind::Dot(exp, _)
        | ExpKind::Iter(exp, _) => is_partial_exp(exp),
        ExpKind::Bin(_, _, exp_l, exp_r)
        | ExpKind::Cmp(_, _, exp_l, exp_r)
        | ExpKind::Cons(exp_l, exp_r)
        | ExpKind::Cat(exp_l, exp_r)
        | ExpKind::Mem(exp_l, exp_r)
        | ExpKind::Idx(exp_l, exp_r) => is_partial_exp(exp_l) || is_partial_exp(exp_r),
        ExpKind::Tuple(exps) | ExpKind::List(exps) => exps.iter().any(is_partial_exp),
        ExpKind::Case(not_exp) => not_exp.args().into_iter().any(is_partial_exp),
        ExpKind::Str(fields) => fields.iter().any(|(_, exp)| is_partial_exp(exp)),
        ExpKind::Opt(exp) => exp.as_deref().is_some_and(is_partial_exp),
        ExpKind::Slice(exp_b, exp_i, exp_n) => {
            is_partial_exp(exp_b) || is_partial_exp(exp_i) || is_partial_exp(exp_n)
        }
        ExpKind::Upd(exp_b, path, exp_f) => {
            is_partial_exp(exp_b) || is_partial_path(path) || is_partial_exp(exp_f)
        }
        ExpKind::Call(..) => true,
    }
}

/// Checks whether a path may fail during evaluation
pub fn is_partial_path(path: &Path) -> bool {
    match &path.node {
        PathKind::Root => false,
        PathKind::Idx(path, exp_i) => is_partial_path(path) || is_partial_exp(exp_i),
        PathKind::Slice(path, exp_i, exp_n) => {
            is_partial_path(path) || is_partial_exp(exp_i) || is_partial_exp(exp_n)
        }
        PathKind::Dot(path, _) => is_partial_path(path),
    }
}

/// Checks whether a premise may fail during evaluation
pub fn is_partial_prem(prem: &Prem) -> bool {
    match &prem.node {
        PremKind::Rule(_) | PremKind::If(_) | PremKind::IfHold(_) | PremKind::IfNotHold(_) => true,
        PremKind::Let(prem) => is_partial_exp(&prem.exp_r),
        PremKind::Iter(prem) => is_partial_prem(&prem.prem),
        PremKind::Debug(prem) => is_partial_exp(&prem.exp),
    }
}
