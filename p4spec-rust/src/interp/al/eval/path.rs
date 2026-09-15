//! AL path access and update evaluation

use crate::{
    lang::{
        al::ast,
        common::source::Span,
        data::value::{Value, get, make},
    },
    runner::{Extern, Interface, RunnerContext},
};

use super::super::{
    AlInterp,
    backtrack::{Backtrack, backtrack, backtrack_from_result},
    context::Context,
};
use super::{expr::eval_exp, ops};

// - Access

fn eval_access_path<Iface: Interface, Exn: Extern>(
    runner: &mut RunnerContext<'_, AlInterp, Iface, Exn>,
    ctx: &Context<'_>,
    value_base: &Value,
    path: &ast::Path,
) -> Backtrack<Value> {
    match &path.node {
        ast::PathKind::Root => Backtrack::Ok(*value_base),
        ast::PathKind::Idx(path, exp_idx) => {
            eval_access_idx_path(runner, ctx, value_base, path, exp_idx)
        }
        ast::PathKind::Slice(path, exp_idx, exp_len) => {
            eval_access_slice_path(runner, ctx, value_base, path, exp_idx, exp_len)
        }
        ast::PathKind::Dot(path, atom) => eval_access_dot_path(runner, ctx, value_base, path, atom),
    }
}

// - Index access path

fn eval_access_idx_path<Iface: Interface, Exn: Extern>(
    runner: &mut RunnerContext<'_, AlInterp, Iface, Exn>,
    ctx: &Context<'_>,
    value_base: &Value,
    path: &ast::Path,
    exp_idx: &ast::Exp,
) -> Backtrack<Value> {
    let value = backtrack!(eval_access_path(runner, ctx, value_base, path));
    let value_idx = backtrack!(eval_exp(runner, ctx, exp_idx));
    ops::access_index(
        runner.arena_mut(),
        &value,
        &value_idx,
        &path.span,
        &exp_idx.span,
    )
}

// - Slice access path

fn eval_access_slice_path<Iface: Interface, Exn: Extern>(
    runner: &mut RunnerContext<'_, AlInterp, Iface, Exn>,
    ctx: &Context<'_>,
    value_base: &Value,
    path: &ast::Path,
    exp_idx: &ast::Exp,
    exp_len: &ast::Exp,
) -> Backtrack<Value> {
    let typ = &path.note;
    let value = backtrack!(eval_access_path(runner, ctx, value_base, path));
    let value_idx = backtrack!(eval_exp(runner, ctx, exp_idx));
    let value_len = backtrack!(eval_exp(runner, ctx, exp_len));
    ops::access_slice(
        runner.arena_mut(),
        &value,
        &value_idx,
        &value_len,
        typ,
        &path.span,
        &path.span,
        &exp_idx.span,
        &exp_len.span,
        &exp_len.span,
    )
}

// - Field access path

fn eval_access_dot_path<Iface: Interface, Exn: Extern>(
    runner: &mut RunnerContext<'_, AlInterp, Iface, Exn>,
    ctx: &Context<'_>,
    value_base: &Value,
    path: &ast::Path,
    atom: &ast::Atom,
) -> Backtrack<Value> {
    let value = backtrack!(eval_access_path(runner, ctx, value_base, path));
    ops::access_dot(runner.arena(), &value, atom, &path.span)
}

// - Update

pub(super) fn eval_update_path<Iface: Interface, Exn: Extern>(
    runner: &mut RunnerContext<'_, AlInterp, Iface, Exn>,
    ctx: &Context<'_>,
    value_base: &Value,
    path: &ast::Path,
    value_upd: Value,
) -> Backtrack<Value> {
    match &path.node {
        ast::PathKind::Root => Backtrack::Ok(value_upd),
        ast::PathKind::Idx(path, exp_idx) => {
            eval_update_idx_path(runner, ctx, value_base, path, exp_idx, value_upd)
        }
        ast::PathKind::Slice(path, exp_idx, exp_len) => {
            eval_update_slice_path(runner, ctx, value_base, path, exp_idx, exp_len, value_upd)
        }
        ast::PathKind::Dot(path, atom) => {
            eval_update_dot_path(runner, ctx, value_base, path, atom, value_upd)
        }
    }
}

// - Index update path

fn eval_update_idx_path<Iface: Interface, Exn: Extern>(
    runner: &mut RunnerContext<'_, AlInterp, Iface, Exn>,
    ctx: &Context<'_>,
    value_base: &Value,
    path: &ast::Path,
    exp_idx: &ast::Exp,
    value_upd: Value,
) -> Backtrack<Value> {
    let typ = crate::phrase!(node: path.note.clone(), span: path.span.clone());
    let value = backtrack!(eval_access_path(runner, ctx, value_base, path));
    let value_idx = backtrack!(eval_exp(runner, ctx, exp_idx));
    let value = backtrack!(ops::update_index(
        runner.arena_mut(),
        &value,
        &value_idx,
        value_upd,
        &typ,
        &path.span,
        &exp_idx.span
    ));
    eval_update_path(runner, ctx, value_base, path, value)
}

// - Slice update path

fn eval_update_slice_path<Iface: Interface, Exn: Extern>(
    runner: &mut RunnerContext<'_, AlInterp, Iface, Exn>,
    ctx: &Context<'_>,
    value_base: &Value,
    path: &ast::Path,
    exp_idx: &ast::Exp,
    exp_len: &ast::Exp,
    value_upd: Value,
) -> Backtrack<Value> {
    let typ = crate::phrase!(node: path.note.clone(), span: path.span.clone());
    let value = backtrack!(eval_access_path(runner, ctx, value_base, path));
    let value_idx = backtrack!(eval_exp(runner, ctx, exp_idx));
    let value_len = backtrack!(eval_exp(runner, ctx, exp_len));
    let value = backtrack!(ops::update_slice(
        runner.arena_mut(),
        &value,
        &value_idx,
        &value_len,
        value_upd,
        &typ,
        &path.span,
        &exp_idx.span,
        &exp_len.span
    ));
    eval_update_path(runner, ctx, value_base, path, value)
}

// - Field update path

fn eval_update_dot_path<Iface: Interface, Exn: Extern>(
    runner: &mut RunnerContext<'_, AlInterp, Iface, Exn>,
    ctx: &Context<'_>,
    value_base: &Value,
    path: &ast::Path,
    atom: &ast::Atom,
    value_upd: Value,
) -> Backtrack<Value> {
    let typ = crate::phrase!(node: path.note.clone(), span: path.span.clone());
    let value = backtrack!(eval_access_path(runner, ctx, value_base, path));
    let value_fields = backtrack_from_result!(get::structure(runner.arena(), &value), &path.span);
    let value_fields = value_fields
        .iter()
        .map(|(field, value)| {
            (
                field.clone(),
                if field.node == atom.node {
                    value_upd
                } else {
                    *value
                },
            )
        })
        .collect();
    let value = backtrack_from_result!(
        make::structure(
            runner.arena_mut(),
            typ.node.clone(),
            value_fields,
            Span::default()
        ),
        &Span::default()
    );
    eval_update_path(runner, ctx, value_base, path, value)
}
