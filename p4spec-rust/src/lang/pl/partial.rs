//! Partial prose-language constructs
//!
//! A construct is partial when its evaluation can fail, that is, when it calls
//! a relation or function that may not match.

use super::ast as pl;

/// Reports whether evaluation of a path can fail.
pub fn is_partial_path(path: &pl::Path) -> bool {
    match &path.node {
        pl::PathKind::Root => false,
        pl::PathKind::Idx(path_base, exp_idx) => {
            is_partial_path(path_base) || is_partial_exp(exp_idx)
        }
        pl::PathKind::Slice(path_base, exp_idx, exp_len) => {
            is_partial_path(path_base) || is_partial_exp(exp_idx) || is_partial_exp(exp_len)
        }
        pl::PathKind::Dot(path_base, _) => is_partial_path(path_base),
    }
}

/// Reports whether evaluation of an expression can fail.
pub fn is_partial_exp(exp: &pl::Exp) -> bool {
    use pl::ExpKind;
    match &exp.node.node {
        ExpKind::Bool(_) | ExpKind::Num(_) | ExpKind::Text(_) | ExpKind::Id(_) => false,
        ExpKind::Un(_, _, exp_inner)
        | ExpKind::UpCast(_, exp_inner)
        | ExpKind::DownCast(_, exp_inner)
        | ExpKind::Sub(exp_inner, _, _)
        | ExpKind::Match(exp_inner, _)
        | ExpKind::Len(exp_inner)
        | ExpKind::Dot(exp_inner, _)
        | ExpKind::Iter(exp_inner, _) => is_partial_exp(exp_inner),
        ExpKind::Bin(_, _, exp_l, exp_r)
        | ExpKind::Cmp(_, _, exp_l, exp_r)
        | ExpKind::Cons(exp_l, exp_r)
        | ExpKind::Cat(exp_l, exp_r)
        | ExpKind::Mem(exp_l, exp_r)
        | ExpKind::Idx(exp_l, exp_r) => is_partial_exp(exp_l) || is_partial_exp(exp_r),
        ExpKind::Tuple(exps) | ExpKind::List(exps) => exps.iter().any(is_partial_exp),
        ExpKind::Case(not_exp) => not_exp.args().into_iter().any(is_partial_exp),
        ExpKind::Str(fields) => fields.iter().any(|(_, exp)| is_partial_exp(exp)),
        ExpKind::Opt(exp_opt) => exp_opt.as_deref().is_some_and(is_partial_exp),
        ExpKind::Slice(exp_base, exp_idx, exp_len) => {
            is_partial_exp(exp_base) || is_partial_exp(exp_idx) || is_partial_exp(exp_len)
        }
        ExpKind::Upd(exp_base, path, exp_field) => {
            is_partial_exp(exp_base) || is_partial_path(path) || is_partial_exp(exp_field)
        }
        ExpKind::Call(..) => true,
    }
}

pub fn is_partial_guard(guard: &pl::Guard) -> bool {
    match guard {
        pl::Guard::Bool(_) | pl::Guard::Sub(..) | pl::Guard::Match(_) => false,
        pl::Guard::Cmp(_, _, exp)
        | pl::Guard::Mem(exp)
        | pl::Guard::CheckLetSub(_, _, exp)
        | pl::Guard::CheckLetMatch(_, exp) => is_partial_exp(exp),
    }
}
