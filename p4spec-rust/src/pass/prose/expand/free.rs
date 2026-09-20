//! Dimension-aware free variables for call lifting

use crate::lang::{il::ast as il, sl::ast as sl};

pub(super) fn var_eq(var_a: &sl::Var, var_b: &sl::Var) -> bool {
    var_a.id.node == var_b.id.node && var_a.iters == var_b.iters
}

pub(super) fn extend_vars(vars: &mut Vec<sl::Var>, vars_other: impl IntoIterator<Item = sl::Var>) {
    for var in vars_other {
        if !vars.iter().any(|var_other| var_eq(var_other, &var)) {
            vars.push(var);
        }
    }
}

pub(super) fn free_vars_arg(arg_sl: &sl::Arg) -> Vec<sl::Var> {
    match &arg_sl.node {
        il::ArgKind::Exp(exp_sl) => free_vars_exp(exp_sl),
        il::ArgKind::Def(_) => Vec::new(),
    }
}

pub(super) fn free_vars_args(args_sl: &[sl::Arg]) -> Vec<sl::Var> {
    let mut vars = Vec::new();
    for arg_sl in args_sl {
        extend_vars(&mut vars, free_vars_arg(arg_sl));
    }
    vars
}

pub(super) fn free_vars_path(path_sl: &sl::Path) -> Vec<sl::Var> {
    match &path_sl.node {
        il::PathKind::Root => Vec::new(),
        il::PathKind::Idx(path_inner_sl, exp_sl) => {
            let mut vars = free_vars_path(path_inner_sl);
            extend_vars(&mut vars, free_vars_exp(exp_sl));
            vars
        }
        il::PathKind::Slice(path_inner_sl, exp_idx_sl, exp_len_sl) => {
            let mut vars = free_vars_path(path_inner_sl);
            extend_vars(&mut vars, free_vars_exp(exp_idx_sl));
            extend_vars(&mut vars, free_vars_exp(exp_len_sl));
            vars
        }
        il::PathKind::Dot(path_inner_sl, _) => free_vars_path(path_inner_sl),
    }
}

pub(super) fn free_vars_exp(exp_sl: &sl::Exp) -> Vec<sl::Var> {
    match &exp_sl.node {
        il::ExpKind::Bool(_) | il::ExpKind::Num(_) | il::ExpKind::Text(_) => Vec::new(),
        il::ExpKind::Var(id) => vec![sl::Var {
            id: id.clone(),
            typ: crate::phrase! {
                node: exp_sl.note.as_ref().clone(),
                span: exp_sl.span.clone(),
            },
            iters: Vec::new(),
        }],
        il::ExpKind::Un(_, _, exp_inner_sl)
        | il::ExpKind::UpCast(_, exp_inner_sl)
        | il::ExpKind::DownCast(_, exp_inner_sl)
        | il::ExpKind::Sub(exp_inner_sl, _, _)
        | il::ExpKind::Match(exp_inner_sl, _)
        | il::ExpKind::Len(exp_inner_sl)
        | il::ExpKind::Dot(exp_inner_sl, _) => free_vars_exp(exp_inner_sl),
        il::ExpKind::Bin(_, _, exp_l_sl, exp_r_sl)
        | il::ExpKind::Cmp(_, _, exp_l_sl, exp_r_sl)
        | il::ExpKind::Cons(exp_l_sl, exp_r_sl)
        | il::ExpKind::Cat(exp_l_sl, exp_r_sl)
        | il::ExpKind::Mem(exp_l_sl, exp_r_sl)
        | il::ExpKind::Idx(exp_l_sl, exp_r_sl) => {
            let mut vars = free_vars_exp(exp_l_sl);
            extend_vars(&mut vars, free_vars_exp(exp_r_sl));
            vars
        }
        il::ExpKind::Tuple(exps_sl) | il::ExpKind::List(exps_sl) => {
            let mut vars = Vec::new();
            for exp_sl in exps_sl {
                extend_vars(&mut vars, free_vars_exp(exp_sl));
            }
            vars
        }
        il::ExpKind::Case(not_exp_sl) => {
            let mut vars = Vec::new();
            for exp_sl in not_exp_sl.args() {
                extend_vars(&mut vars, free_vars_exp(exp_sl));
            }
            vars
        }
        il::ExpKind::Str(fields_sl) => {
            let mut vars = Vec::new();
            for (_, exp_sl) in fields_sl {
                extend_vars(&mut vars, free_vars_exp(exp_sl));
            }
            vars
        }
        il::ExpKind::Opt(exp_opt_sl) => {
            exp_opt_sl.as_deref().map(free_vars_exp).unwrap_or_default()
        }
        il::ExpKind::Slice(exp_base_sl, exp_idx_sl, exp_len_sl) => {
            let mut vars = free_vars_exp(exp_base_sl);
            extend_vars(&mut vars, free_vars_exp(exp_idx_sl));
            extend_vars(&mut vars, free_vars_exp(exp_len_sl));
            vars
        }
        il::ExpKind::Upd(exp_base_sl, path_sl, exp_field_sl) => {
            let mut vars = free_vars_exp(exp_base_sl);
            extend_vars(&mut vars, free_vars_path(path_sl));
            extend_vars(&mut vars, free_vars_exp(exp_field_sl));
            vars
        }
        il::ExpKind::Call(_, _, args_sl) => free_vars_args(args_sl),
        il::ExpKind::Iter(exp_inner_sl, (iter, vars_bound)) => {
            let mut vars = free_vars_exp(exp_inner_sl);
            for var in &mut vars {
                if vars_bound.iter().any(|var_bound| var_eq(var_bound, var)) {
                    var.iters.push(*iter);
                }
            }
            vars
        }
    }
}
