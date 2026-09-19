//! Lift nested calls into explicit SL let instructions

use crate::lang::{
    common::ds::{map::IdMap, set::IdSet},
    hints::input,
    il::{self, ast as il_ast},
    sl::ast as sl,
    traits::free::Free,
};

use super::{ProseError, ProseErrorKind};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum CallCount {
    Yes,
    No,
    SkipOne,
}

struct Binding {
    exp_l: sl::Exp,
    exp_r: sl::Exp,
    vars_inner: Vec<sl::Var>,
    vars_outer: Vec<sl::Var>,
    var_new: sl::Var,
    iter_exps: Vec<sl::ExpIter>,
    iter_instrs: Vec<sl::InstrIter>,
}

fn var_eq(var_a: &sl::Var, var_b: &sl::Var) -> bool {
    var_a.id.node == var_b.id.node && var_a.iters == var_b.iters
}

fn vars_insert(vars: &mut Vec<sl::Var>, var: sl::Var) {
    if !vars.iter().any(|var_other| var_eq(var_other, &var)) {
        vars.push(var);
    }
}

fn vars_extend(vars: &mut Vec<sl::Var>, vars_other: impl IntoIterator<Item = sl::Var>) {
    for var in vars_other {
        vars_insert(vars, var);
    }
}

fn free_vars_arg(arg_sl: &sl::Arg) -> Vec<sl::Var> {
    match &arg_sl.node {
        il_ast::ArgKind::Exp(exp_sl) => free_vars_exp(exp_sl),
        il_ast::ArgKind::Def(_) => Vec::new(),
    }
}

fn free_vars_args(args_sl: &[sl::Arg]) -> Vec<sl::Var> {
    let mut vars = Vec::new();
    for arg_sl in args_sl {
        vars_extend(&mut vars, free_vars_arg(arg_sl));
    }
    vars
}

fn free_vars_path(path_sl: &sl::Path) -> Vec<sl::Var> {
    match &path_sl.node {
        il_ast::PathKind::Root => Vec::new(),
        il_ast::PathKind::Idx(path_inner_sl, exp_sl) => {
            let mut vars = free_vars_path(path_inner_sl);
            vars_extend(&mut vars, free_vars_exp(exp_sl));
            vars
        }
        il_ast::PathKind::Slice(path_inner_sl, exp_l_sl, exp_h_sl) => {
            let mut vars = free_vars_path(path_inner_sl);
            vars_extend(&mut vars, free_vars_exp(exp_l_sl));
            vars_extend(&mut vars, free_vars_exp(exp_h_sl));
            vars
        }
        il_ast::PathKind::Dot(path_inner_sl, _) => free_vars_path(path_inner_sl),
    }
}

fn free_vars_exp(exp_sl: &sl::Exp) -> Vec<sl::Var> {
    match &exp_sl.node {
        il_ast::ExpKind::Bool(_) | il_ast::ExpKind::Num(_) | il_ast::ExpKind::Text(_) => Vec::new(),
        il_ast::ExpKind::Var(id) => vec![sl::Var {
            id: id.clone(),
            typ: crate::phrase! {
                node: exp_sl.note.as_ref().clone(),
                span: exp_sl.span.clone(),
            },
            iters: Vec::new(),
        }],
        il_ast::ExpKind::Un(_, _, exp_inner_sl)
        | il_ast::ExpKind::UpCast(_, exp_inner_sl)
        | il_ast::ExpKind::DownCast(_, exp_inner_sl)
        | il_ast::ExpKind::Sub(exp_inner_sl, _, _)
        | il_ast::ExpKind::Match(exp_inner_sl, _)
        | il_ast::ExpKind::Len(exp_inner_sl)
        | il_ast::ExpKind::Dot(exp_inner_sl, _) => free_vars_exp(exp_inner_sl),
        il_ast::ExpKind::Bin(_, _, exp_l_sl, exp_r_sl)
        | il_ast::ExpKind::Cmp(_, _, exp_l_sl, exp_r_sl)
        | il_ast::ExpKind::Cons(exp_l_sl, exp_r_sl)
        | il_ast::ExpKind::Cat(exp_l_sl, exp_r_sl)
        | il_ast::ExpKind::Mem(exp_l_sl, exp_r_sl)
        | il_ast::ExpKind::Idx(exp_l_sl, exp_r_sl) => {
            let mut vars = free_vars_exp(exp_l_sl);
            vars_extend(&mut vars, free_vars_exp(exp_r_sl));
            vars
        }
        il_ast::ExpKind::Tuple(exps_sl) | il_ast::ExpKind::List(exps_sl) => {
            let mut vars = Vec::new();
            for exp_sl in exps_sl {
                vars_extend(&mut vars, free_vars_exp(exp_sl));
            }
            vars
        }
        il_ast::ExpKind::Case(not_exp_sl) => {
            let mut vars = Vec::new();
            for exp_sl in not_exp_sl.args() {
                vars_extend(&mut vars, free_vars_exp(exp_sl));
            }
            vars
        }
        il_ast::ExpKind::Str(fields_sl) => {
            let mut vars = Vec::new();
            for (_, exp_sl) in fields_sl {
                vars_extend(&mut vars, free_vars_exp(exp_sl));
            }
            vars
        }
        il_ast::ExpKind::Opt(exp_opt_sl) => {
            exp_opt_sl.as_deref().map(free_vars_exp).unwrap_or_default()
        }
        il_ast::ExpKind::Slice(exp_base_sl, exp_l_sl, exp_h_sl) => {
            let mut vars = free_vars_exp(exp_base_sl);
            vars_extend(&mut vars, free_vars_exp(exp_l_sl));
            vars_extend(&mut vars, free_vars_exp(exp_h_sl));
            vars
        }
        il_ast::ExpKind::Upd(exp_base_sl, path_sl, exp_field_sl) => {
            let mut vars = free_vars_exp(exp_base_sl);
            vars_extend(&mut vars, free_vars_path(path_sl));
            vars_extend(&mut vars, free_vars_exp(exp_field_sl));
            vars
        }
        il_ast::ExpKind::Call(_, _, args_sl) => free_vars_args(args_sl),
        il_ast::ExpKind::Iter(exp_inner_sl, (iter, vars_bound)) => {
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

fn add_outer_vars(mut binding: Binding, vars: Vec<sl::Var>) -> Binding {
    vars_extend(&mut binding.vars_outer, vars);
    binding
}

fn count_call(count: CallCount, exp: &sl::Exp) -> CallCount {
    match exp.node {
        il_ast::ExpKind::Call(..) => {
            if count == CallCount::No {
                CallCount::SkipOne
            } else {
                CallCount::Yes
            }
        }
        il_ast::ExpKind::Iter(..) => count,
        _ => CallCount::Yes,
    }
}

fn args_reference_local(iter_locals: &IdSet, args: &[sl::Arg]) -> bool {
    args.free()
        .iter()
        .any(|id| iter_locals.iter().any(|id_local| id_local.node == id.node))
}

fn extract_root_call(
    exp_target_sl: &mut sl::Exp,
    count: CallCount,
    iter_locals: &IdSet,
    ids_used: &mut IdSet,
) -> Option<Binding> {
    let il_ast::ExpKind::Call(_, _, args) = &exp_target_sl.node else {
        return None;
    };
    if count != CallCount::Yes || args.is_empty() || args_reference_local(iter_locals, args) {
        return None;
    }

    let typ = crate::phrase! {
        node: exp_target_sl.note.as_ref().clone(),
        span: exp_target_sl.span.clone(),
    };
    let var = il::fresh::var_from_typ(&IdMap::new(), ids_used, exp_target_sl.span.clone(), &typ);
    ids_used.insert(var.id.clone());
    let exp_new_sl = il::var::as_exp(true, &var);
    let exp_orig_sl = std::mem::replace(exp_target_sl, exp_new_sl.clone());
    let il_ast::ExpKind::Call(_, _, args) = &exp_orig_sl.node else {
        unreachable!();
    };
    let vars_inner = free_vars_args(args);
    Some(Binding {
        exp_l: exp_new_sl,
        exp_r: exp_orig_sl,
        vars_inner,
        vars_outer: Vec::new(),
        var_new: var,
        iter_exps: Vec::new(),
        iter_instrs: Vec::new(),
    })
}

fn extract_exps(
    exps_sl: &mut [sl::Exp],
    count: CallCount,
    iter_locals: &IdSet,
    ids_used: &mut IdSet,
) -> Option<Binding> {
    for index in 0..exps_sl.len() {
        if let Some(binding) = extract_exp(&mut exps_sl[index], count, iter_locals, ids_used) {
            let mut vars_outer = Vec::new();
            for (index_other, exp_other_sl) in exps_sl.iter().enumerate() {
                if index_other != index {
                    vars_extend(&mut vars_outer, free_vars_exp(exp_other_sl));
                }
            }
            return Some(add_outer_vars(binding, vars_outer));
        }
    }
    None
}

fn extract_args(
    args_sl: &mut [sl::Arg],
    count: CallCount,
    iter_locals: &IdSet,
    ids_used: &mut IdSet,
) -> Option<Binding> {
    for index in 0..args_sl.len() {
        if let il_ast::ArgKind::Exp(exp_sl) = &mut args_sl[index].node
            && let Some(binding) = extract_exp(exp_sl, count, iter_locals, ids_used)
        {
            let mut vars_outer = Vec::new();
            for (index_other, arg_other_sl) in args_sl.iter().enumerate() {
                if index_other != index {
                    vars_extend(&mut vars_outer, free_vars_arg(arg_other_sl));
                }
            }
            return Some(add_outer_vars(binding, vars_outer));
        }
    }
    None
}

fn extract_path(
    path_sl: &mut sl::Path,
    count: CallCount,
    iter_locals: &IdSet,
    ids_used: &mut IdSet,
) -> Option<Binding> {
    match &mut path_sl.node {
        il_ast::PathKind::Root => None,
        il_ast::PathKind::Idx(path_inner_sl, exp_idx_sl) => {
            if let Some(binding) = extract_path(path_inner_sl, count, iter_locals, ids_used) {
                Some(add_outer_vars(binding, free_vars_exp(exp_idx_sl)))
            } else {
                extract_exp(exp_idx_sl, count, iter_locals, ids_used)
                    .map(|binding| add_outer_vars(binding, free_vars_path(path_inner_sl)))
            }
        }
        il_ast::PathKind::Slice(path_inner_sl, exp_idx_sl, exp_len_sl) => {
            if let Some(binding) = extract_path(path_inner_sl, count, iter_locals, ids_used) {
                let mut vars_outer = free_vars_exp(exp_idx_sl);
                vars_extend(&mut vars_outer, free_vars_exp(exp_len_sl));
                Some(add_outer_vars(binding, vars_outer))
            } else if let Some(binding) = extract_exp(exp_idx_sl, count, iter_locals, ids_used) {
                let mut vars_outer = free_vars_path(path_inner_sl);
                vars_extend(&mut vars_outer, free_vars_exp(exp_len_sl));
                Some(add_outer_vars(binding, vars_outer))
            } else {
                extract_exp(exp_len_sl, count, iter_locals, ids_used).map(|binding| {
                    let mut vars_outer = free_vars_path(path_inner_sl);
                    vars_extend(&mut vars_outer, free_vars_exp(exp_idx_sl));
                    add_outer_vars(binding, vars_outer)
                })
            }
        }
        il_ast::PathKind::Dot(path_inner_sl, _) => {
            extract_path(path_inner_sl, count, iter_locals, ids_used)
        }
    }
}

fn extract_exp(
    exp_target_sl: &mut sl::Exp,
    count: CallCount,
    iter_locals: &IdSet,
    ids_used: &mut IdSet,
) -> Option<Binding> {
    let count = count_call(count, exp_target_sl);
    if let Some(binding) = extract_root_call(exp_target_sl, count, iter_locals, ids_used) {
        return Some(binding);
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
            extract_exp(exp_inner_sl, count, iter_locals, ids_used)
        }
        il_ast::ExpKind::Bin(_, _, exp_l_sl, exp_r_sl)
        | il_ast::ExpKind::Cmp(_, _, exp_l_sl, exp_r_sl)
        | il_ast::ExpKind::Cons(exp_l_sl, exp_r_sl)
        | il_ast::ExpKind::Cat(exp_l_sl, exp_r_sl)
        | il_ast::ExpKind::Mem(exp_l_sl, exp_r_sl)
        | il_ast::ExpKind::Idx(exp_l_sl, exp_r_sl) => {
            if let Some(binding) = extract_exp(exp_l_sl, count, iter_locals, ids_used) {
                Some(add_outer_vars(binding, free_vars_exp(exp_r_sl)))
            } else {
                extract_exp(exp_r_sl, count, iter_locals, ids_used)
                    .map(|binding| add_outer_vars(binding, free_vars_exp(exp_l_sl)))
            }
        }
        il_ast::ExpKind::Tuple(exps_sl) | il_ast::ExpKind::List(exps_sl) => {
            extract_exps(exps_sl, count, iter_locals, ids_used)
        }
        il_ast::ExpKind::Case(not_exp_sl) => {
            let mut exps_sl = not_exp_sl.args().into_iter().cloned().collect::<Vec<_>>();
            let binding = extract_exps(&mut exps_sl, count, iter_locals, ids_used)?;
            let mut exps_sl = exps_sl.into_iter();
            **not_exp_sl = not_exp_sl.map(|_| exps_sl.next().unwrap());
            Some(binding)
        }
        il_ast::ExpKind::Str(fields_sl) => {
            for index in 0..fields_sl.len() {
                if let Some(binding) =
                    extract_exp(&mut fields_sl[index].1, count, iter_locals, ids_used)
                {
                    let mut vars_outer = Vec::new();
                    for (index_other, (_, exp_other_sl)) in fields_sl.iter().enumerate() {
                        if index_other != index {
                            vars_extend(&mut vars_outer, free_vars_exp(exp_other_sl));
                        }
                    }
                    return Some(add_outer_vars(binding, vars_outer));
                }
            }
            None
        }
        il_ast::ExpKind::Opt(exp_opt_sl) => exp_opt_sl
            .as_deref_mut()
            .and_then(|exp_sl| extract_exp(exp_sl, count, iter_locals, ids_used)),
        il_ast::ExpKind::Slice(exp_base_sl, exp_idx_sl, exp_len_sl) => {
            if let Some(binding) = extract_exp(exp_base_sl, count, iter_locals, ids_used) {
                let mut vars_outer = free_vars_exp(exp_idx_sl);
                vars_extend(&mut vars_outer, free_vars_exp(exp_len_sl));
                Some(add_outer_vars(binding, vars_outer))
            } else if let Some(binding) = extract_exp(exp_idx_sl, count, iter_locals, ids_used) {
                let mut vars_outer = free_vars_exp(exp_base_sl);
                vars_extend(&mut vars_outer, free_vars_exp(exp_len_sl));
                Some(add_outer_vars(binding, vars_outer))
            } else {
                extract_exp(exp_len_sl, count, iter_locals, ids_used).map(|binding| {
                    let mut vars_outer = free_vars_exp(exp_base_sl);
                    vars_extend(&mut vars_outer, free_vars_exp(exp_idx_sl));
                    add_outer_vars(binding, vars_outer)
                })
            }
        }
        il_ast::ExpKind::Upd(exp_base_sl, path_sl, exp_field_sl) => {
            if let Some(binding) = extract_exp(exp_base_sl, count, iter_locals, ids_used) {
                let mut vars_outer = free_vars_path(path_sl);
                vars_extend(&mut vars_outer, free_vars_exp(exp_field_sl));
                Some(add_outer_vars(binding, vars_outer))
            } else if let Some(binding) = extract_path(path_sl, count, iter_locals, ids_used) {
                let mut vars_outer = free_vars_exp(exp_base_sl);
                vars_extend(&mut vars_outer, free_vars_exp(exp_field_sl));
                Some(add_outer_vars(binding, vars_outer))
            } else {
                extract_exp(exp_field_sl, count, iter_locals, ids_used).map(|binding| {
                    let mut vars_outer = free_vars_exp(exp_base_sl);
                    vars_extend(&mut vars_outer, free_vars_path(path_sl));
                    add_outer_vars(binding, vars_outer)
                })
            }
        }
        il_ast::ExpKind::Call(_, _, args_sl) => extract_args(args_sl, count, iter_locals, ids_used),
        il_ast::ExpKind::Iter(exp_inner_sl, (iter, vars_bound)) => {
            let mut binding = extract_exp(exp_inner_sl, count, iter_locals, ids_used)?;
            let vars_matched = binding
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
                return Some(binding);
            }

            for var_inner in &mut binding.vars_inner {
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
                        || binding
                            .vars_outer
                            .iter()
                            .any(|var_outer| var_eq(var_outer, var_bound))
                })
                .cloned()
                .collect::<Vec<_>>();
            let var_new_inner = binding.var_new.clone();
            binding.var_new.iters.push(*iter);
            binding.iter_exps.push((*iter, vars_iter));
            *vars_bound = std::iter::once(var_new_inner).chain(vars_kept).collect();
            Some(binding)
        }
    }
}

fn ids_of_vars<'a>(vars: impl IntoIterator<Item = &'a sl::Var>) -> IdSet {
    vars.into_iter().map(|var| var.id.clone()).collect()
}

fn iter_locals_instr(iter_instrs: &[sl::InstrIter]) -> IdSet {
    iter_instrs
        .iter()
        .flat_map(|iter_instr| iter_instr.vars_bound.iter())
        .map(|var| var.id.clone())
        .collect()
}

fn iter_locals_exp(iter_exps: &[sl::ExpIter]) -> IdSet {
    ids_of_vars(iter_exps.iter().flat_map(|(_, vars)| vars))
}

fn extract_first_exp(
    exp_sl: &mut sl::Exp,
    count: CallCount,
    iter_locals: &IdSet,
    ids_used: &mut IdSet,
) -> Option<Binding> {
    extract_exp(exp_sl, count, iter_locals, ids_used)
}

fn extract_first_exps(
    exps_sl: &mut [sl::Exp],
    count: CallCount,
    iter_locals: &IdSet,
    ids_used: &mut IdSet,
) -> Option<Binding> {
    extract_exps(exps_sl, count, iter_locals, ids_used)
}

fn extract_instr(
    instr_sl: &mut sl::Instr,
    ids_used: &mut IdSet,
) -> Result<Option<Binding>, ProseError> {
    let span = instr_sl.span.clone();
    Ok(match &mut instr_sl.node {
        sl::InstrKind::Let(instr_sl) => extract_first_exp(
            &mut instr_sl.exp_r,
            CallCount::No,
            &iter_locals_instr(&instr_sl.iter_instrs),
            ids_used,
        ),
        sl::InstrKind::Rule(instr_sl) => {
            let exps_sl = instr_sl
                .not_exp
                .args()
                .into_iter()
                .cloned()
                .collect::<Vec<_>>();
            let (mut exps_input_sl, exps_output_sl) =
                input::split(&instr_sl.input_hint, exps_sl)
                    .map_err(|error| ProseError::new(ProseErrorKind::Input(error), span.clone()))?;
            let binding = extract_first_exps(
                &mut exps_input_sl,
                CallCount::SkipOne,
                &iter_locals_instr(&instr_sl.iter_instrs),
                ids_used,
            );
            if binding.is_some() {
                let exps_sl =
                    input::combine(&instr_sl.input_hint, exps_input_sl, exps_output_sl)
                        .map_err(|error| ProseError::new(ProseErrorKind::Input(error), span))?;
                let mut exps_sl = exps_sl.into_iter();
                instr_sl.not_exp = instr_sl.not_exp.map(|_| exps_sl.next().unwrap());
            }
            binding
        }
        sl::InstrKind::Hold(instr_sl) => {
            let mut exps_sl = instr_sl
                .not_exp
                .args()
                .into_iter()
                .cloned()
                .collect::<Vec<_>>();
            let binding = extract_first_exps(
                &mut exps_sl,
                CallCount::SkipOne,
                &iter_locals_exp(&instr_sl.iter_exps),
                ids_used,
            );
            if binding.is_some() {
                let mut exps_sl = exps_sl.into_iter();
                instr_sl.not_exp = instr_sl.not_exp.map(|_| exps_sl.next().unwrap());
            }
            binding
        }
        sl::InstrKind::Result(instr_sl) => {
            extract_first_exps(&mut instr_sl.exps, CallCount::No, &IdSet::new(), ids_used)
        }
        sl::InstrKind::Return(instr_sl) => {
            extract_first_exp(&mut instr_sl.exp, CallCount::No, &IdSet::new(), ids_used)
        }
        sl::InstrKind::If(_)
        | sl::InstrKind::Case(_)
        | sl::InstrKind::Group(_)
        | sl::InstrKind::Debug(_) => None,
    })
}

fn expand_hold_case(
    ids_used: &mut IdSet,
    hold_case_sl: sl::HoldCase,
) -> Result<sl::HoldCase, ProseError> {
    Ok(match hold_case_sl {
        sl::HoldCase::Both(block_hold_sl, block_not_hold_sl) => sl::HoldCase::Both(
            expand_block(ids_used, block_hold_sl)?,
            expand_block(ids_used, block_not_hold_sl)?,
        ),
        sl::HoldCase::Hold(block_sl, dangle) => {
            sl::HoldCase::Hold(expand_block(ids_used, block_sl)?, dangle)
        }
        sl::HoldCase::NotHold(block_sl, dangle) => {
            sl::HoldCase::NotHold(expand_block(ids_used, block_sl)?, dangle)
        }
    })
}

fn expand_subblocks(ids_used: &mut IdSet, instr_sl: sl::Instr) -> Result<sl::Instr, ProseError> {
    let span = instr_sl.span;
    let instr_kind_sl = match instr_sl.node {
        sl::InstrKind::Let(mut instr_sl) => {
            instr_sl.block = expand_block(ids_used, instr_sl.block)?;
            sl::InstrKind::Let(instr_sl)
        }
        sl::InstrKind::Rule(mut instr_sl) => {
            instr_sl.block = expand_block(ids_used, instr_sl.block)?;
            sl::InstrKind::Rule(instr_sl)
        }
        sl::InstrKind::If(mut instr_sl) => {
            instr_sl.block = expand_block(ids_used, instr_sl.block)?;
            sl::InstrKind::If(instr_sl)
        }
        sl::InstrKind::Hold(mut instr_sl) => {
            instr_sl.hold_case = expand_hold_case(ids_used, instr_sl.hold_case)?;
            sl::InstrKind::Hold(instr_sl)
        }
        sl::InstrKind::Case(mut instr_sl) => {
            for case_sl in &mut instr_sl.cases {
                case_sl.block = expand_block(ids_used, std::mem::take(&mut case_sl.block))?;
            }
            sl::InstrKind::Case(instr_sl)
        }
        sl::InstrKind::Group(mut instr_sl) => {
            instr_sl.block = expand_block(ids_used, instr_sl.block)?;
            sl::InstrKind::Group(instr_sl)
        }
        sl::InstrKind::Debug(mut instr_sl) => {
            instr_sl.instr = Box::new(expand_instr(ids_used, *instr_sl.instr)?);
            sl::InstrKind::Debug(instr_sl)
        }
        sl::InstrKind::Result(instr_sl) => sl::InstrKind::Result(instr_sl),
        sl::InstrKind::Return(instr_sl) => sl::InstrKind::Return(instr_sl),
    };
    Ok(crate::phrase! { node: instr_kind_sl, span: span })
}

fn wrap_binding(binding: Binding, instr_sl: sl::Instr) -> sl::Instr {
    let Binding { exp_l, exp_r, var_new, iter_exps, mut iter_instrs, .. } = binding;
    let enclosing_count = iter_exps.len();
    let callee_count = var_new.iters.len() - enclosing_count;
    let mut var_bind =
        sl::Var { id: var_new.id, typ: var_new.typ, iters: var_new.iters[..callee_count].to_vec() };
    for (iter, vars_bound) in iter_exps {
        iter_instrs.push(sl::InstrIter { iter, vars_bound, vars_bind: vec![var_bind.clone()] });
        var_bind.iters.push(iter);
    }
    let span = instr_sl.span.clone();
    crate::phrase! {
        node: sl::InstrKind::Let(sl::LetInstr {
            exp_l,
            exp_r,
            iter_instrs,
            block: vec![instr_sl],
        }),
        span: span,
    }
}

fn expand_instr(ids_used: &mut IdSet, mut instr_sl: sl::Instr) -> Result<sl::Instr, ProseError> {
    instr_sl = expand_subblocks(ids_used, instr_sl)?;
    let mut bindings = Vec::new();
    while let Some(binding) = extract_instr(&mut instr_sl, ids_used)? {
        bindings.push(binding);
    }
    if bindings.is_empty() {
        return Ok(instr_sl);
    }
    for binding in bindings.into_iter().rev() {
        instr_sl = wrap_binding(binding, instr_sl);
    }
    expand_instr(ids_used, instr_sl)
}

fn expand_block(ids_used: &mut IdSet, block_sl: sl::Block) -> Result<sl::Block, ProseError> {
    block_sl
        .into_iter()
        .map(|instr_sl| expand_instr(ids_used, instr_sl))
        .collect()
}

fn expand_def(mut def_sl: sl::Def) -> Result<sl::Def, ProseError> {
    match &mut def_sl.node {
        sl::DefKind::Rel(sl::RelDef::Defined(def_rel_sl)) => {
            let mut ids_used = def_rel_sl.exps_input.free().union(def_rel_sl.block.free());
            if let Some(block_else_sl) = &def_rel_sl.block_else {
                ids_used.append(block_else_sl.free());
            }
            def_rel_sl.block = expand_block(&mut ids_used, std::mem::take(&mut def_rel_sl.block))?;
            if let Some(block_else_sl) = def_rel_sl.block_else.take() {
                def_rel_sl.block_else = Some(expand_block(&mut ids_used, block_else_sl)?);
            }
        }
        sl::DefKind::MetaFunc(sl::MetaFuncDef::Table(def_func_sl)) => {
            for row_sl in &mut def_func_sl.table_rows {
                let mut ids_used = row_sl
                    .exps_input
                    .free()
                    .union(row_sl.exp.free())
                    .union(row_sl.block.free());
                row_sl.block = expand_block(&mut ids_used, std::mem::take(&mut row_sl.block))?;
            }
        }
        sl::DefKind::MetaFunc(sl::MetaFuncDef::Defined(def_func_sl)) => {
            let mut ids_used = def_func_sl.params.free().union(def_func_sl.block.free());
            if let Some(block_else_sl) = &def_func_sl.block_else {
                ids_used.append(block_else_sl.free());
            }
            def_func_sl.block =
                expand_block(&mut ids_used, std::mem::take(&mut def_func_sl.block))?;
            if let Some(block_else_sl) = def_func_sl.block_else.take() {
                def_func_sl.block_else = Some(expand_block(&mut ids_used, block_else_sl)?);
            }
        }
        _ => {}
    }
    Ok(def_sl)
}

pub(super) fn spec(spec_sl: sl::Spec) -> Result<sl::Spec, ProseError> {
    spec_sl.into_iter().map(expand_def).collect()
}
