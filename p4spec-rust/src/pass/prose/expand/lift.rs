//! Lift the first eligible call from an SL expression

use crate::lang::{
    common::ds::{map::IdMap, set::IdSet},
    il::{self, ast as il_ast},
    sl::ast as sl,
    traits::free::Free,
};

use super::free::{
    extend_vars, free_vars_arg, free_vars_args, free_vars_exp, free_vars_path, var_eq,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum RootCallPolicy {
    Preserve,
    Lift,
}

pub(super) struct LiftedCall {
    exp_replacement_sl: sl::Exp,
    exp_call_sl: sl::Exp,
    vars_inner: Vec<sl::Var>,
    vars_outer: Vec<sl::Var>,
    var_fresh: sl::Var,
    iter_exps: Vec<sl::ExpIter>,
}

impl LiftedCall {
    fn with_outer_vars(mut self, vars_outer: Vec<sl::Var>) -> Self {
        extend_vars(&mut self.vars_outer, vars_outer);
        self
    }

    pub(super) fn wrap_instr(self, instr_sl: sl::Instr) -> sl::Instr {
        let Self { exp_replacement_sl, exp_call_sl, var_fresh, iter_exps, .. } = self;
        let num_iters_enclosing = iter_exps.len();
        let num_iters_callee = var_fresh.iters.len() - num_iters_enclosing;
        let mut var_bind = sl::Var {
            id: var_fresh.id,
            typ: var_fresh.typ,
            iters: var_fresh.iters[..num_iters_callee].to_vec(),
        };
        let mut iter_instrs = Vec::new();
        for (iter, vars_bound) in iter_exps {
            iter_instrs.push(sl::InstrIter { iter, vars_bound, vars_bind: vec![var_bind.clone()] });
            var_bind.iters.push(iter);
        }
        let span = instr_sl.span.clone();
        crate::phrase! {
            node: sl::InstrKind::Let(sl::LetInstr {
                exp_l: exp_replacement_sl,
                exp_r: exp_call_sl,
                iter_instrs,
                block: vec![instr_sl],
            }),
            span: span,
        }
    }
}

fn args_reference_local(ids_iter_local: &IdSet, args_sl: &[sl::Arg]) -> bool {
    args_sl.free().iter().any(|id| ids_iter_local.contains(id))
}

fn lift_root_call(
    exp_target_sl: &mut sl::Exp,
    ids_iter_local: &IdSet,
    ids_used: &mut IdSet,
) -> Option<LiftedCall> {
    let il_ast::ExpKind::Call(_, _, args_sl) = &exp_target_sl.node else {
        return None;
    };
    if args_sl.is_empty() || args_reference_local(ids_iter_local, args_sl) {
        return None;
    }

    let typ = crate::phrase! {
        node: exp_target_sl.note.as_ref().clone(),
        span: exp_target_sl.span.clone(),
    };
    let var_fresh =
        il::fresh::var_from_typ(&IdMap::new(), ids_used, exp_target_sl.span.clone(), &typ);
    ids_used.insert(var_fresh.id.clone());
    let exp_replacement_sl = il::var::as_exp(true, &var_fresh);
    let exp_call_sl = std::mem::replace(exp_target_sl, exp_replacement_sl.clone());
    let il_ast::ExpKind::Call(_, _, args_sl) = &exp_call_sl.node else {
        unreachable!();
    };
    let vars_inner = free_vars_args(args_sl);
    Some(LiftedCall {
        exp_replacement_sl,
        exp_call_sl,
        vars_inner,
        vars_outer: Vec::new(),
        var_fresh,
        iter_exps: Vec::new(),
    })
}

pub(super) fn lift_first_exps(
    exps_sl: &mut [sl::Exp],
    root_call_policy: RootCallPolicy,
    ids_iter_local: &IdSet,
    ids_used: &mut IdSet,
) -> Option<LiftedCall> {
    for idx in 0..exps_sl.len() {
        if let Some(lifted_call) =
            lift_first_exp(&mut exps_sl[idx], root_call_policy, ids_iter_local, ids_used)
        {
            let mut vars_outer = Vec::new();
            for (idx_other, exp_other_sl) in exps_sl.iter().enumerate() {
                if idx_other != idx {
                    extend_vars(&mut vars_outer, free_vars_exp(exp_other_sl));
                }
            }
            return Some(lifted_call.with_outer_vars(vars_outer));
        }
    }
    None
}

fn lift_first_args(
    args_sl: &mut [sl::Arg],
    ids_iter_local: &IdSet,
    ids_used: &mut IdSet,
) -> Option<LiftedCall> {
    for idx in 0..args_sl.len() {
        if let il_ast::ArgKind::Exp(exp_sl) = &mut args_sl[idx].node
            && let Some(lifted_call) =
                lift_first_exp(exp_sl, RootCallPolicy::Lift, ids_iter_local, ids_used)
        {
            let mut vars_outer = Vec::new();
            for (idx_other, arg_other_sl) in args_sl.iter().enumerate() {
                if idx_other != idx {
                    extend_vars(&mut vars_outer, free_vars_arg(arg_other_sl));
                }
            }
            return Some(lifted_call.with_outer_vars(vars_outer));
        }
    }
    None
}

fn lift_first_path(
    path_sl: &mut sl::Path,
    ids_iter_local: &IdSet,
    ids_used: &mut IdSet,
) -> Option<LiftedCall> {
    match &mut path_sl.node {
        il_ast::PathKind::Root => None,
        il_ast::PathKind::Idx(path_inner_sl, exp_idx_sl) => {
            if let Some(lifted_call) = lift_first_path(path_inner_sl, ids_iter_local, ids_used) {
                Some(lifted_call.with_outer_vars(free_vars_exp(exp_idx_sl)))
            } else {
                lift_first_exp(exp_idx_sl, RootCallPolicy::Lift, ids_iter_local, ids_used)
                    .map(|lifted_call| lifted_call.with_outer_vars(free_vars_path(path_inner_sl)))
            }
        }
        il_ast::PathKind::Slice(path_inner_sl, exp_idx_sl, exp_len_sl) => {
            if let Some(lifted_call) = lift_first_path(path_inner_sl, ids_iter_local, ids_used) {
                let mut vars_outer = free_vars_exp(exp_idx_sl);
                extend_vars(&mut vars_outer, free_vars_exp(exp_len_sl));
                Some(lifted_call.with_outer_vars(vars_outer))
            } else if let Some(lifted_call) =
                lift_first_exp(exp_idx_sl, RootCallPolicy::Lift, ids_iter_local, ids_used)
            {
                let mut vars_outer = free_vars_path(path_inner_sl);
                extend_vars(&mut vars_outer, free_vars_exp(exp_len_sl));
                Some(lifted_call.with_outer_vars(vars_outer))
            } else {
                lift_first_exp(exp_len_sl, RootCallPolicy::Lift, ids_iter_local, ids_used).map(
                    |lifted_call| {
                        let mut vars_outer = free_vars_path(path_inner_sl);
                        extend_vars(&mut vars_outer, free_vars_exp(exp_idx_sl));
                        lifted_call.with_outer_vars(vars_outer)
                    },
                )
            }
        }
        il_ast::PathKind::Dot(path_inner_sl, _) => {
            lift_first_path(path_inner_sl, ids_iter_local, ids_used)
        }
    }
}

pub(super) fn lift_first_exp(
    exp_target_sl: &mut sl::Exp,
    root_call_policy: RootCallPolicy,
    ids_iter_local: &IdSet,
    ids_used: &mut IdSet,
) -> Option<LiftedCall> {
    if root_call_policy == RootCallPolicy::Lift
        && let Some(lifted_call) = lift_root_call(exp_target_sl, ids_iter_local, ids_used)
    {
        return Some(lifted_call);
    }

    match &mut exp_target_sl.node {
        il_ast::ExpKind::Bool(_)
        | il_ast::ExpKind::Num(_)
        | il_ast::ExpKind::Text(_)
        | il_ast::ExpKind::Var(_) => None,
        il_ast::ExpKind::Un(_, _, exp_inner_sl)
        | il_ast::ExpKind::UpCast(_, exp_inner_sl)
        | il_ast::ExpKind::DownCast(_, exp_inner_sl)
        | il_ast::ExpKind::Sub(exp_inner_sl, _, _)
        | il_ast::ExpKind::Match(exp_inner_sl, _)
        | il_ast::ExpKind::Len(exp_inner_sl)
        | il_ast::ExpKind::Dot(exp_inner_sl, _) => {
            lift_first_exp(exp_inner_sl, RootCallPolicy::Lift, ids_iter_local, ids_used)
        }
        il_ast::ExpKind::Bin(_, _, exp_l_sl, exp_r_sl)
        | il_ast::ExpKind::Cmp(_, _, exp_l_sl, exp_r_sl)
        | il_ast::ExpKind::Cons(exp_l_sl, exp_r_sl)
        | il_ast::ExpKind::Cat(exp_l_sl, exp_r_sl)
        | il_ast::ExpKind::Mem(exp_l_sl, exp_r_sl)
        | il_ast::ExpKind::Idx(exp_l_sl, exp_r_sl) => {
            if let Some(lifted_call) =
                lift_first_exp(exp_l_sl, RootCallPolicy::Lift, ids_iter_local, ids_used)
            {
                Some(lifted_call.with_outer_vars(free_vars_exp(exp_r_sl)))
            } else {
                lift_first_exp(exp_r_sl, RootCallPolicy::Lift, ids_iter_local, ids_used)
                    .map(|lifted_call| lifted_call.with_outer_vars(free_vars_exp(exp_l_sl)))
            }
        }
        il_ast::ExpKind::Tuple(exps_sl) | il_ast::ExpKind::List(exps_sl) => {
            lift_first_exps(exps_sl, RootCallPolicy::Lift, ids_iter_local, ids_used)
        }
        il_ast::ExpKind::Case(not_exp_sl) => {
            let mut exps_sl = not_exp_sl.args().into_iter().cloned().collect::<Vec<_>>();
            let lifted_call =
                lift_first_exps(&mut exps_sl, RootCallPolicy::Lift, ids_iter_local, ids_used)?;
            let mut exps_sl = exps_sl.into_iter();
            **not_exp_sl = not_exp_sl.map(|_| exps_sl.next().unwrap());
            Some(lifted_call)
        }
        il_ast::ExpKind::Str(fields_sl) => {
            for idx in 0..fields_sl.len() {
                if let Some(lifted_call) = lift_first_exp(
                    &mut fields_sl[idx].1,
                    RootCallPolicy::Lift,
                    ids_iter_local,
                    ids_used,
                ) {
                    let mut vars_outer = Vec::new();
                    for (idx_other, (_, exp_other_sl)) in fields_sl.iter().enumerate() {
                        if idx_other != idx {
                            extend_vars(&mut vars_outer, free_vars_exp(exp_other_sl));
                        }
                    }
                    return Some(lifted_call.with_outer_vars(vars_outer));
                }
            }
            None
        }
        il_ast::ExpKind::Opt(exp_opt_sl) => exp_opt_sl.as_deref_mut().and_then(|exp_sl| {
            lift_first_exp(exp_sl, RootCallPolicy::Lift, ids_iter_local, ids_used)
        }),
        il_ast::ExpKind::Slice(exp_base_sl, exp_idx_sl, exp_len_sl) => {
            if let Some(lifted_call) =
                lift_first_exp(exp_base_sl, RootCallPolicy::Lift, ids_iter_local, ids_used)
            {
                let mut vars_outer = free_vars_exp(exp_idx_sl);
                extend_vars(&mut vars_outer, free_vars_exp(exp_len_sl));
                Some(lifted_call.with_outer_vars(vars_outer))
            } else if let Some(lifted_call) =
                lift_first_exp(exp_idx_sl, RootCallPolicy::Lift, ids_iter_local, ids_used)
            {
                let mut vars_outer = free_vars_exp(exp_base_sl);
                extend_vars(&mut vars_outer, free_vars_exp(exp_len_sl));
                Some(lifted_call.with_outer_vars(vars_outer))
            } else {
                lift_first_exp(exp_len_sl, RootCallPolicy::Lift, ids_iter_local, ids_used).map(
                    |lifted_call| {
                        let mut vars_outer = free_vars_exp(exp_base_sl);
                        extend_vars(&mut vars_outer, free_vars_exp(exp_idx_sl));
                        lifted_call.with_outer_vars(vars_outer)
                    },
                )
            }
        }
        il_ast::ExpKind::Upd(exp_base_sl, path_sl, exp_field_sl) => {
            if let Some(lifted_call) =
                lift_first_exp(exp_base_sl, RootCallPolicy::Lift, ids_iter_local, ids_used)
            {
                let mut vars_outer = free_vars_path(path_sl);
                extend_vars(&mut vars_outer, free_vars_exp(exp_field_sl));
                Some(lifted_call.with_outer_vars(vars_outer))
            } else if let Some(lifted_call) = lift_first_path(path_sl, ids_iter_local, ids_used) {
                let mut vars_outer = free_vars_exp(exp_base_sl);
                extend_vars(&mut vars_outer, free_vars_exp(exp_field_sl));
                Some(lifted_call.with_outer_vars(vars_outer))
            } else {
                lift_first_exp(exp_field_sl, RootCallPolicy::Lift, ids_iter_local, ids_used).map(
                    |lifted_call| {
                        let mut vars_outer = free_vars_exp(exp_base_sl);
                        extend_vars(&mut vars_outer, free_vars_path(path_sl));
                        lifted_call.with_outer_vars(vars_outer)
                    },
                )
            }
        }
        il_ast::ExpKind::Call(_, _, args_sl) => lift_first_args(args_sl, ids_iter_local, ids_used),
        il_ast::ExpKind::Iter(exp_inner_sl, (iter, vars_bound)) => {
            let mut lifted_call =
                lift_first_exp(exp_inner_sl, root_call_policy, ids_iter_local, ids_used)?;
            let vars_matched = lifted_call
                .vars_inner
                .iter()
                .filter(|var_inner| {
                    vars_bound
                        .iter()
                        .any(|var_bound| var_eq(var_bound, var_inner))
                })
                .cloned()
                .collect::<Vec<_>>();
            if vars_matched.is_empty() {
                return Some(lifted_call);
            }

            for var_inner in &mut lifted_call.vars_inner {
                if vars_matched
                    .iter()
                    .any(|var_matched| var_eq(var_matched, var_inner))
                {
                    var_inner.iters.push(*iter);
                }
            }
            let vars_iter = vars_bound
                .iter()
                .filter(|var_bound| {
                    vars_matched
                        .iter()
                        .any(|var_matched| var_eq(var_matched, var_bound))
                })
                .cloned()
                .collect::<Vec<_>>();
            let vars_kept = vars_bound
                .iter()
                .filter(|var_bound| {
                    !vars_matched
                        .iter()
                        .any(|var_matched| var_eq(var_matched, var_bound))
                        || lifted_call
                            .vars_outer
                            .iter()
                            .any(|var_outer| var_eq(var_outer, var_bound))
                })
                .cloned()
                .collect::<Vec<_>>();
            let var_fresh_inner = lifted_call.var_fresh.clone();
            lifted_call.var_fresh.iters.push(*iter);
            lifted_call.iter_exps.push((*iter, vars_iter));
            *vars_bound = std::iter::once(var_fresh_inner).chain(vars_kept).collect();
            Some(lifted_call)
        }
    }
}

pub(super) fn ids_bound_by_instr_iters(iter_instrs: &[sl::InstrIter]) -> IdSet {
    iter_instrs
        .iter()
        .flat_map(|iter_instr| iter_instr.vars_bound.iter())
        .map(|var| var.id.clone())
        .collect()
}

pub(super) fn ids_bound_by_exp_iters(iter_exps: &[sl::ExpIter]) -> IdSet {
    iter_exps
        .iter()
        .flat_map(|(_, vars)| vars)
        .map(|var| var.id.clone())
        .collect()
}
