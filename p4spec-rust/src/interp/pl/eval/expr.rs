//! Shared expression evaluation
//!
//! Re-exports evaluation of prepared expressions; PL adds no expression forms.

pub(super) use crate::interp::shared::eval::expr::{eval_exp, eval_exps};
