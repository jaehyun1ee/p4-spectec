//! Detection of calls contained in prose-language syntax
//!
//! Expressions and paths recurse through every evaluated child.

use crate::lang::traits::has_call::HasCall;

use super::ast::{ExpKind, PathKind};

// == Expressions

impl HasCall for ExpKind {
    fn has_call(&self) -> bool {
        match self {
            Self::Bool(_) | Self::Num(_) | Self::Text(_) | Self::Var(_) => false,
            Self::Un(_, _, exp)
            | Self::UpCast(_, exp)
            | Self::DownCast(_, exp)
            | Self::Sub(exp, _, _)
            | Self::Match(exp, _)
            | Self::Len(exp)
            | Self::Dot(exp, _)
            | Self::Iter(exp, _) => exp.has_call(),
            Self::Bin(_, _, exp_l, exp_r)
            | Self::Cmp(_, _, exp_l, exp_r)
            | Self::Cons(exp_l, exp_r)
            | Self::Cat(exp_l, exp_r)
            | Self::Mem(exp_l, exp_r)
            | Self::Idx(exp_l, exp_r) => exp_l.has_call() || exp_r.has_call(),
            Self::Tuple(exps) | Self::List(exps) => exps.iter().any(HasCall::has_call),
            Self::Case(not_exp) => not_exp.args().into_iter().any(HasCall::has_call),
            Self::Str(fields) => fields.iter().any(|(_, exp)| exp.has_call()),
            Self::Opt(exp_opt) => exp_opt.as_deref().is_some_and(HasCall::has_call),
            Self::Slice(exp_base, exp_idx, exp_len) => {
                exp_base.has_call() || exp_idx.has_call() || exp_len.has_call()
            }
            Self::Upd(exp_base, path, exp_field) => {
                exp_base.has_call() || path.has_call() || exp_field.has_call()
            }
            Self::Call(..) => true,
        }
    }
}

// == Paths

impl HasCall for PathKind {
    fn has_call(&self) -> bool {
        match self {
            Self::Root => false,
            Self::Idx(path, exp_idx) => path.has_call() || exp_idx.has_call(),
            Self::Slice(path, exp_idx, exp_len) => {
                path.has_call() || exp_idx.has_call() || exp_len.has_call()
            }
            Self::Dot(path, _) => path.has_call(),
        }
    }
}
