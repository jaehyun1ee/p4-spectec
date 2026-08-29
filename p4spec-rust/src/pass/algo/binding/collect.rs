//! Binding collection through invertible expression positions

use crate::lang::{
    common::source::{Span, Spanned},
    il::ast,
};

use super::{
    super::{AlgoError, AlgoErrorKind},
    bind::{self, Bindings},
    context::Context,
};

fn reject_noninvertible(
    span: Span,
    construct: &'static str,
    bindings: Bindings,
) -> Result<Bindings, AlgoError> {
    if bindings.is_empty() {
        Ok(bindings)
    } else {
        Err(AlgoError::new(
            AlgoErrorKind::NonInvertibleBinding(construct),
            span,
        ))
    }
}

fn collect_exp_refs<'a>(
    ctx: &Context,
    exps: impl IntoIterator<Item = &'a ast::Exp>,
) -> Result<Bindings, AlgoError> {
    let mut bindings = Bindings::new();
    for exp in exps {
        bindings = bind::union(bindings, collect_exp(ctx, exp)?)?;
    }
    Ok(bindings)
}

pub fn collect_exp(ctx: &Context, exp: &ast::Exp) -> Result<Bindings, AlgoError> {
    match &exp.node.kind {
        ast::ExpKind::Bool(_) | ast::ExpKind::Num(_) | ast::ExpKind::Text(_) => Ok(Bindings::new()),
        ast::ExpKind::Var(id) => {
            if ctx.venv.contains_key(id) {
                Ok(Bindings::new())
            } else {
                let typ = Spanned::new(exp.node.note.clone(), exp.span.clone());
                Ok(bind::singleton(id.clone(), typ))
            }
        }
        ast::ExpKind::Un(_, _, exp_inner) => {
            let bindings = collect_exp(ctx, exp_inner)?;
            reject_noninvertible(exp_inner.span.clone(), "unary operator", bindings)
        }
        ast::ExpKind::Bin(_, _, exp_l, exp_r) => {
            let bindings_l = collect_exp(ctx, exp_l)?;
            let bindings_r = collect_exp(ctx, exp_r)?;
            let bindings = bind::union(bindings_l, bindings_r)?;
            reject_noninvertible(exp.span.clone(), "binary operator", bindings)
        }
        ast::ExpKind::Cmp(_, _, exp_l, exp_r) => {
            let bindings_l = collect_exp(ctx, exp_l)?;
            let bindings_r = collect_exp(ctx, exp_r)?;
            let bindings = bind::union(bindings_l, bindings_r)?;
            reject_noninvertible(exp.span.clone(), "comparison operator", bindings)
        }
        ast::ExpKind::UpCast(_, exp_inner) => collect_exp(ctx, exp_inner),
        ast::ExpKind::DownCast(_, exp_inner) => {
            let bindings = collect_exp(ctx, exp_inner)?;
            reject_noninvertible(exp_inner.span.clone(), "downcast operator", bindings)
        }
        ast::ExpKind::Sub(exp_inner, _, _) => {
            let bindings = collect_exp(ctx, exp_inner)?;
            reject_noninvertible(exp_inner.span.clone(), "subtype check operator", bindings)
        }
        ast::ExpKind::Match(exp_inner, _) => {
            let bindings = collect_exp(ctx, exp_inner)?;
            reject_noninvertible(exp_inner.span.clone(), "match check operator", bindings)
        }
        ast::ExpKind::Tuple(exps) | ast::ExpKind::List(exps) => collect_exps(ctx, exps),
        ast::ExpKind::Case(not_exp) => collect_exp_refs(ctx, not_exp.args()),
        ast::ExpKind::Str(fields) => collect_exp_refs(ctx, fields.iter().map(|(_, exp)| exp)),
        ast::ExpKind::Opt(Some(exp_inner)) => collect_exp(ctx, exp_inner),
        ast::ExpKind::Opt(None) => Ok(Bindings::new()),
        ast::ExpKind::Cons(exp_l, exp_r) => {
            let bindings_l = collect_exp(ctx, exp_l)?;
            let bindings_r = collect_exp(ctx, exp_r)?;
            bind::union(bindings_l, bindings_r)
        }
        ast::ExpKind::Cat(exp_l, exp_r) => {
            let bindings_l = collect_exp(ctx, exp_l)?;
            let bindings_r = collect_exp(ctx, exp_r)?;
            let bindings = bind::union(bindings_l, bindings_r)?;
            reject_noninvertible(exp.span.clone(), "concatenation operator", bindings)
        }
        ast::ExpKind::Mem(exp_l, exp_r) => {
            let bindings_l = collect_exp(ctx, exp_l)?;
            let bindings_r = collect_exp(ctx, exp_r)?;
            let bindings = bind::union(bindings_l, bindings_r)?;
            reject_noninvertible(exp.span.clone(), "set membership operator", bindings)
        }
        ast::ExpKind::Len(exp_inner) => {
            let bindings = collect_exp(ctx, exp_inner)?;
            reject_noninvertible(exp_inner.span.clone(), "length operator", bindings)
        }
        ast::ExpKind::Dot(exp_inner, _) => {
            let bindings = collect_exp(ctx, exp_inner)?;
            reject_noninvertible(exp_inner.span.clone(), "dot operator", bindings)
        }
        ast::ExpKind::Idx(exp_base, exp_index) => {
            let bindings_base = collect_exp(ctx, exp_base)?;
            let bindings_index = collect_exp(ctx, exp_index)?;
            let bindings = bind::union(bindings_base, bindings_index)?;
            reject_noninvertible(exp.span.clone(), "indexing operator", bindings)
        }
        ast::ExpKind::Slice(exp_base, exp_l, exp_h) => {
            let bindings_base = collect_exp(ctx, exp_base)?;
            let bindings_l = collect_exp(ctx, exp_l)?;
            let bindings_h = collect_exp(ctx, exp_h)?;
            let bindings = bind::union(bindings_base, bindings_l)?;
            let bindings = bind::union(bindings, bindings_h)?;
            reject_noninvertible(exp.span.clone(), "slicing operator", bindings)
        }
        ast::ExpKind::Upd(exp_base, path, exp_field) => {
            let bindings_base = collect_exp(ctx, exp_base)?;
            let bindings_path = collect_path(ctx, path)?;
            let bindings_field = collect_exp(ctx, exp_field)?;
            let bindings = bind::union(bindings_base, bindings_field)?;
            let bindings = bind::union(bindings, bindings_path)?;
            reject_noninvertible(exp.span.clone(), "update operator", bindings)
        }
        ast::ExpKind::Call(_, _, args) => {
            let bindings = collect_args(ctx, args)?;
            reject_noninvertible(exp.span.clone(), "call operator", bindings)
        }
        ast::ExpKind::Iter(exp_inner, (iter, _)) => {
            let bindings = collect_exp(ctx, exp_inner)?;
            Ok(bind::add_iter(bindings, *iter))
        }
    }
}

pub fn collect_exps(ctx: &Context, exps: &[ast::Exp]) -> Result<Bindings, AlgoError> {
    collect_exp_refs(ctx, exps)
}

pub fn collect_path(ctx: &Context, path: &ast::Path) -> Result<Bindings, AlgoError> {
    match &path.node.kind {
        ast::PathKind::Root => Ok(Bindings::new()),
        ast::PathKind::Idx(path, exp) => {
            let bindings_path = collect_path(ctx, path)?;
            let bindings_exp = collect_exp(ctx, exp)?;
            bind::union(bindings_path, bindings_exp)
        }
        ast::PathKind::Slice(path, exp_l, exp_h) => {
            let bindings_path = collect_path(ctx, path)?;
            let bindings_l = collect_exp(ctx, exp_l)?;
            let bindings_h = collect_exp(ctx, exp_h)?;
            let bindings = bind::union(bindings_path, bindings_l)?;
            bind::union(bindings, bindings_h)
        }
        ast::PathKind::Dot(path, _) => collect_path(ctx, path),
    }
}

pub fn collect_arg(ctx: &Context, arg: &ast::Arg) -> Result<Bindings, AlgoError> {
    match &arg.node {
        ast::ArgKind::Exp(exp) => collect_exp(ctx, exp),
        ast::ArgKind::Def(_) => Ok(Bindings::new()),
    }
}

pub fn collect_args(ctx: &Context, args: &[ast::Arg]) -> Result<Bindings, AlgoError> {
    let mut bindings = Bindings::new();
    for arg in args {
        bindings = bind::union(bindings, collect_arg(ctx, arg)?)?;
    }
    Ok(bindings)
}
