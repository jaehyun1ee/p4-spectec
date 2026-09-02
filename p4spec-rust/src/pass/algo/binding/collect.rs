//! Binding collection through invertible expression positions.
//!
//! Free identifiers become binders only while traversing invertible constructs. A binder
//! found below any non-invertible operation is rejected at that operation's source span.

use crate::lang::{common::source::Span, il::ast};

use super::{
    super::{AlgoError, AlgoErrorKind},
    bind::BEnv,
    context::Context,
};

// == Helpers

// - Errors

fn reject_noninvertible(
    span: Span,
    construct: &'static str,
    benv: BEnv,
) -> Result<BEnv, AlgoError> {
    if benv.is_empty() {
        Ok(benv)
    } else {
        let error = AlgoError::new(AlgoErrorKind::NonInvertibleBinding(construct), span);
        Err(error)
    }
}

// == Binding collection

// - Expressions

pub fn collect_exp(ctx: &Context, exp: &ast::Exp) -> Result<BEnv, AlgoError> {
    match &exp.node {
        ast::ExpKind::Bool(_) | ast::ExpKind::Num(_) | ast::ExpKind::Text(_) => {
            let benv = BEnv::new();
            Ok(benv)
        }
        ast::ExpKind::Var(id) => {
            if ctx.venv.contains_key(id) {
                let benv = BEnv::new();
                Ok(benv)
            } else {
                let typ = ast::typ_from_note(&exp.note, exp.span.clone());
                let benv = BEnv::singleton(id.clone(), typ);
                Ok(benv)
            }
        }
        ast::ExpKind::Un(_, _, exp_inner) => {
            let benv = collect_exp(ctx, exp_inner)?;
            reject_noninvertible(exp_inner.span.clone(), "unary operator", benv)
        }
        ast::ExpKind::Bin(_, _, exp_l, exp_r) => {
            let benv_l = collect_exp(ctx, exp_l)?;
            let benv_r = collect_exp(ctx, exp_r)?;
            let benv = benv_l.union(benv_r)?;
            reject_noninvertible(exp.span.clone(), "binary operator", benv)
        }
        ast::ExpKind::Cmp(_, _, exp_l, exp_r) => {
            let benv_l = collect_exp(ctx, exp_l)?;
            let benv_r = collect_exp(ctx, exp_r)?;
            let benv = benv_l.union(benv_r)?;
            reject_noninvertible(exp.span.clone(), "comparison operator", benv)
        }
        ast::ExpKind::UpCast(_, exp_inner) => collect_exp(ctx, exp_inner),
        ast::ExpKind::DownCast(_, exp_inner) => {
            let benv = collect_exp(ctx, exp_inner)?;
            reject_noninvertible(exp_inner.span.clone(), "downcast operator", benv)
        }
        ast::ExpKind::Sub(exp_inner, _, _) => {
            let benv = collect_exp(ctx, exp_inner)?;
            reject_noninvertible(exp_inner.span.clone(), "subtype check operator", benv)
        }
        ast::ExpKind::Match(exp_inner, _) => {
            let benv = collect_exp(ctx, exp_inner)?;
            reject_noninvertible(exp_inner.span.clone(), "match check operator", benv)
        }
        ast::ExpKind::Tuple(exps) | ast::ExpKind::List(exps) => collect_exps(ctx, exps),
        ast::ExpKind::Case(not_exp) => collect_exps(ctx, not_exp.args()),
        ast::ExpKind::Str(fields) => collect_exps(ctx, fields.iter().map(|(_, exp)| exp)),
        ast::ExpKind::Opt(Some(exp_inner)) => collect_exp(ctx, exp_inner),
        ast::ExpKind::Opt(None) => {
            let benv = BEnv::new();
            Ok(benv)
        }
        ast::ExpKind::Cons(exp_l, exp_r) => {
            let benv_l = collect_exp(ctx, exp_l)?;
            let benv_r = collect_exp(ctx, exp_r)?;
            benv_l.union(benv_r)
        }
        ast::ExpKind::Cat(exp_l, exp_r) => {
            let benv_l = collect_exp(ctx, exp_l)?;
            let benv_r = collect_exp(ctx, exp_r)?;
            let benv = benv_l.union(benv_r)?;
            reject_noninvertible(exp.span.clone(), "concatenation operator", benv)
        }
        ast::ExpKind::Mem(exp_l, exp_r) => {
            let benv_l = collect_exp(ctx, exp_l)?;
            let benv_r = collect_exp(ctx, exp_r)?;
            let benv = benv_l.union(benv_r)?;
            reject_noninvertible(exp.span.clone(), "set membership operator", benv)
        }
        ast::ExpKind::Len(exp_inner) => {
            let benv = collect_exp(ctx, exp_inner)?;
            reject_noninvertible(exp_inner.span.clone(), "length operator", benv)
        }
        ast::ExpKind::Dot(exp_inner, _) => {
            let benv = collect_exp(ctx, exp_inner)?;
            reject_noninvertible(exp_inner.span.clone(), "dot operator", benv)
        }
        ast::ExpKind::Idx(exp_base, exp_index) => {
            let benv_base = collect_exp(ctx, exp_base)?;
            let benv_index = collect_exp(ctx, exp_index)?;
            let benv = benv_base.union(benv_index)?;
            reject_noninvertible(exp.span.clone(), "indexing operator", benv)
        }
        ast::ExpKind::Slice(exp_base, exp_l, exp_h) => {
            let benv_base = collect_exp(ctx, exp_base)?;
            let benv_l = collect_exp(ctx, exp_l)?;
            let benv_h = collect_exp(ctx, exp_h)?;
            let benv = benv_base.union(benv_l)?;
            let benv = benv.union(benv_h)?;
            reject_noninvertible(exp.span.clone(), "slicing operator", benv)
        }
        ast::ExpKind::Upd(exp_base, path, exp_field) => {
            let benv_base = collect_exp(ctx, exp_base)?;
            let benv_path = collect_path(ctx, path)?;
            let benv_field = collect_exp(ctx, exp_field)?;
            let benv = benv_base.union(benv_field)?;
            let benv = benv.union(benv_path)?;
            reject_noninvertible(exp.span.clone(), "update operator", benv)
        }
        ast::ExpKind::Call(_, _, args) => {
            let benv = collect_args(ctx, args)?;
            reject_noninvertible(exp.span.clone(), "call operator", benv)
        }
        ast::ExpKind::Iter(exp_inner, (iter, _)) => {
            let benv = collect_exp(ctx, exp_inner)?;
            let benv = benv.add_iter(*iter);
            Ok(benv)
        }
    }
}

pub fn collect_exps<'a>(
    ctx: &Context,
    exps: impl IntoIterator<Item = &'a ast::Exp>,
) -> Result<BEnv, AlgoError> {
    let mut benvs = Vec::new();
    for exp in exps {
        benvs.push(collect_exp(ctx, exp)?);
    }
    let mut benv = BEnv::new();
    for benv_head in benvs.into_iter().rev() {
        benv = benv_head.union(benv)?;
    }
    Ok(benv)
}

// - Paths

pub fn collect_path(ctx: &Context, path: &ast::Path) -> Result<BEnv, AlgoError> {
    match &path.node {
        ast::PathKind::Root => {
            let benv = BEnv::new();
            Ok(benv)
        }
        ast::PathKind::Idx(path, exp) => {
            let benv_path = collect_path(ctx, path)?;
            let benv_exp = collect_exp(ctx, exp)?;
            benv_path.union(benv_exp)
        }
        ast::PathKind::Slice(path, exp_l, exp_h) => {
            let benv_path = collect_path(ctx, path)?;
            let benv_l = collect_exp(ctx, exp_l)?;
            let benv_h = collect_exp(ctx, exp_h)?;
            let benv = benv_path.union(benv_l)?;
            benv.union(benv_h)
        }
        ast::PathKind::Dot(path, _) => collect_path(ctx, path),
    }
}

// - Arguments

pub fn collect_arg(ctx: &Context, arg: &ast::Arg) -> Result<BEnv, AlgoError> {
    match &arg.node {
        ast::ArgKind::Exp(exp) => collect_exp(ctx, exp),
        ast::ArgKind::Def(_) => {
            let benv = BEnv::new();
            Ok(benv)
        }
    }
}

pub fn collect_args(ctx: &Context, args: &[ast::Arg]) -> Result<BEnv, AlgoError> {
    let mut benvs = Vec::with_capacity(args.len());
    for arg in args {
        benvs.push(collect_arg(ctx, arg)?);
    }
    let mut benv = BEnv::new();
    for benv_head in benvs.into_iter().rev() {
        benv = benv_head.union(benv)?;
    }
    Ok(benv)
}
