//! SL expressions use the shared IL expression operations

pub(crate) use crate::interp::shared::{
    arg::eval_args,
    assign::{assign_exp, assign_exps},
    expr::{eval_exp, eval_exps},
};
