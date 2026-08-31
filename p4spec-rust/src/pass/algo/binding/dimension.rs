//! Minimal-dimension inference for expression variables.
//!
//! This is the binding pass's reduced form of elaboration dimension inference: it records
//! free variables and retains the minimal dimension when one occurs more than once.

use crate::{
    lang::{common::ds::map::IdMap, il::ast},
    phrase,
    runtime::sta::{Dim, VEnv},
};

fn infer_var(exp: &ast::Exp, id: &ast::Id, iters: &[ast::Iter], venv: &mut VEnv) {
    let typ = phrase!(node: exp.note.clone(), span: exp.span.clone());
    let dim = Dim::new(typ, iters.to_vec());
    if venv
        .get(id)
        .is_none_or(|dim_previous| dim.sub(dim_previous))
    {
        venv.insert(id.clone(), dim);
    }
}

fn infer_exp_inner(exp: &ast::Exp, iters: &[ast::Iter], venv: &mut VEnv) {
    match &exp.node {
        ast::ExpKind::Bool(_) | ast::ExpKind::Num(_) | ast::ExpKind::Text(_) => {}
        ast::ExpKind::Var(id) => infer_var(exp, id, iters, venv),
        ast::ExpKind::Un(_, _, exp)
        | ast::ExpKind::UpCast(_, exp)
        | ast::ExpKind::DownCast(_, exp)
        | ast::ExpKind::Sub(exp, _, _)
        | ast::ExpKind::Match(exp, _)
        | ast::ExpKind::Len(exp)
        | ast::ExpKind::Dot(exp, _) => infer_exp_inner(exp, iters, venv),
        ast::ExpKind::Bin(_, _, exp_l, exp_r)
        | ast::ExpKind::Cmp(_, _, exp_l, exp_r)
        | ast::ExpKind::Cons(exp_l, exp_r)
        | ast::ExpKind::Cat(exp_l, exp_r)
        | ast::ExpKind::Mem(exp_l, exp_r)
        | ast::ExpKind::Idx(exp_l, exp_r) => {
            infer_exp_inner(exp_l, iters, venv);
            infer_exp_inner(exp_r, iters, venv);
        }
        ast::ExpKind::Tuple(exps) | ast::ExpKind::List(exps) => {
            infer_exps_inner(exps, iters, venv);
        }
        ast::ExpKind::Case(not_exp) => {
            for exp in not_exp.args() {
                infer_exp_inner(exp, iters, venv);
            }
        }
        ast::ExpKind::Str(fields) => {
            for (_, exp) in fields {
                infer_exp_inner(exp, iters, venv);
            }
        }
        ast::ExpKind::Opt(Some(exp)) => infer_exp_inner(exp, iters, venv),
        ast::ExpKind::Opt(None) => {}
        ast::ExpKind::Slice(exp_base, exp_l, exp_h) => {
            infer_exp_inner(exp_base, iters, venv);
            infer_exp_inner(exp_l, iters, venv);
            infer_exp_inner(exp_h, iters, venv);
        }
        ast::ExpKind::Upd(exp_base, path, exp_field) => {
            infer_exp_inner(exp_base, iters, venv);
            infer_path_inner(path, iters, venv);
            infer_exp_inner(exp_field, iters, venv);
        }
        ast::ExpKind::Call(_, _, args) => infer_args_inner(args, iters, venv),
        ast::ExpKind::Iter(exp, (iter, _)) => {
            let mut iters_inner = Vec::with_capacity(iters.len() + 1);
            iters_inner.push(*iter);
            iters_inner.extend_from_slice(iters);
            infer_exp_inner(exp, &iters_inner, venv);
        }
    }
}

fn infer_exps_inner(exps: &[ast::Exp], iters: &[ast::Iter], venv: &mut VEnv) {
    for exp in exps {
        infer_exp_inner(exp, iters, venv);
    }
}

fn infer_path_inner(path: &ast::Path, iters: &[ast::Iter], venv: &mut VEnv) {
    match &path.node {
        ast::PathKind::Root => {}
        ast::PathKind::Idx(path, exp) => {
            infer_path_inner(path, iters, venv);
            infer_exp_inner(exp, iters, venv);
        }
        ast::PathKind::Slice(path, exp_l, exp_h) => {
            infer_path_inner(path, iters, venv);
            infer_exp_inner(exp_l, iters, venv);
            infer_exp_inner(exp_h, iters, venv);
        }
        ast::PathKind::Dot(path, _) => infer_path_inner(path, iters, venv),
    }
}

fn infer_args_inner(args: &[ast::Arg], iters: &[ast::Iter], venv: &mut VEnv) {
    for arg in args {
        if let ast::ArgKind::Exp(exp) = &arg.node {
            infer_exp_inner(exp, iters, venv);
        }
    }
}

pub fn infer_exp(exp: &ast::Exp) -> VEnv {
    let mut venv = IdMap::new();
    infer_exp_inner(exp, &[], &mut venv);
    venv
}

pub fn infer_exps(exps: &[ast::Exp]) -> VEnv {
    let mut venv = IdMap::new();
    infer_exps_inner(exps, &[], &mut venv);
    venv
}
