//! AL expression evaluation

use crate::interp::al::error::ExprErrorKind;

use crate::{
    lang::{
        al::ast,
        common::{Variable, source::Span},
        data::value::{Value, ValueKind, get, make},
        il::ast::{ListPattern, OptPattern},
        xl::{bool as boolean, num},
    },
    runner::{Extern, Interface, RunnerContext},
    runtime::ops::{typ::subst_typ, value},
};

use super::super::{
    Al,
    backtrack::{Backtrack, back},
    context::Context,
    error::ErrorKind,
    util::is_iter_var_exp,
};
use super::{arg::eval_args, call::invoke_func, ops, path::eval_update_path};

// = Expression evaluation

pub(super) fn eval_exp<I: Interface, E: Extern>(
    runner: &mut RunnerContext<'_, Al, I, E>,
    ctx: &Context<'_>,
    exp: &ast::Exp,
) -> Backtrack<Value> {
    let span = &exp.span;
    let typ = crate::phrase!(node: exp.note.as_ref().clone(), span: span.clone());
    match &exp.node {
        ast::ExpKind::Bool(value) => Backtrack::Ok(back!(Backtrack::from_result(
            make::bool(runner.arena_mut(), *value, Span::default()),
            &crate::lang::common::source::Span::default()
        ))),
        ast::ExpKind::Num(value) => Backtrack::Ok(back!(Backtrack::from_result(
            make::num(runner.arena_mut(), value.clone(), Span::default()),
            &crate::lang::common::source::Span::default()
        ))),
        ast::ExpKind::Text(value) => Backtrack::Ok(back!(Backtrack::from_result(
            make::text(runner.arena_mut(), value.clone(), Span::default()),
            &crate::lang::common::source::Span::default()
        ))),
        ast::ExpKind::Var(id) => eval_var_exp(ctx, id, span),
        ast::ExpKind::Un(op, _, exp_inner) => eval_un_exp(runner, ctx, op, exp_inner, span),
        ast::ExpKind::Bin(op, _, exp_l, exp_r) => eval_bin_exp(runner, ctx, op, exp_l, exp_r, span),
        ast::ExpKind::Cmp(op, _, exp_l, exp_r) => eval_cmp_exp(runner, ctx, op, exp_l, exp_r, span),
        ast::ExpKind::UpCast(typ, exp_inner) => eval_upcast_exp(runner, ctx, typ, exp_inner),
        ast::ExpKind::DownCast(typ, exp_inner) => eval_downcast_exp(runner, ctx, typ, exp_inner),
        ast::ExpKind::Sub(exp_inner, _, subcheck) => {
            eval_sub_exp(runner, ctx, exp_inner, subcheck, span)
        }
        ast::ExpKind::Match(exp_inner, pattern) => eval_match_exp(runner, ctx, exp_inner, pattern),
        ast::ExpKind::Tuple(exps) => eval_tuple_exp(runner, ctx, exps, &typ),
        ast::ExpKind::Case(not_exp) => eval_case_exp(runner, ctx, not_exp, &typ, span),
        ast::ExpKind::Str(exp_fields) => eval_str_exp(runner, ctx, exp_fields, &typ),
        ast::ExpKind::Opt(exp) => eval_opt_exp(runner, ctx, exp, &typ),
        ast::ExpKind::List(exps) => eval_list_exp(runner, ctx, exps, &typ),
        ast::ExpKind::Cons(exp_head, exp_tail) => {
            eval_cons_exp(runner, ctx, exp_head, exp_tail, &typ, span)
        }
        ast::ExpKind::Cat(exp_l, exp_r) => eval_cat_exp(runner, ctx, exp_l, exp_r, &typ),
        ast::ExpKind::Mem(exp_elem, exp_list) => {
            eval_mem_exp(runner, ctx, exp_elem, exp_list, span)
        }
        ast::ExpKind::Len(exp_inner) => eval_len_exp(runner, ctx, exp_inner),
        ast::ExpKind::Dot(exp_base, atom) => eval_dot_exp(runner, ctx, exp_base, atom, span),
        ast::ExpKind::Idx(exp_base, exp_idx) => eval_idx_exp(runner, ctx, exp_base, exp_idx),
        ast::ExpKind::Slice(exp_base, exp_idx, exp_len) => {
            eval_slice_exp(runner, ctx, exp_base, exp_idx, exp_len, &typ)
        }
        ast::ExpKind::Upd(exp_base, path, exp_upd) => {
            eval_upd_exp(runner, ctx, exp_base, path, exp_upd)
        }
        ast::ExpKind::Call(id, targs, args) => eval_call_exp(runner, ctx, id, targs, args),
        ast::ExpKind::Iter(exp_inner, (iter, vars)) => {
            eval_iter_exp(runner, ctx, exp, exp_inner, iter, vars, &typ)
        }
    }
}

pub(super) fn eval_exps<I: Interface, E: Extern>(
    runner: &mut RunnerContext<'_, Al, I, E>,
    ctx: &Context<'_>,
    exps: &[ast::Exp],
) -> Backtrack<Vec<Value>> {
    let mut values = Vec::with_capacity(exps.len());
    for exp in exps {
        values.push(back!(eval_exp(runner, ctx, exp)));
    }
    Backtrack::Ok(values)
}

// - Variable expression

fn eval_var_exp(ctx: &Context<'_>, id: &ast::Id, span: &Span) -> Backtrack<Value> {
    let var = Variable::new(id.clone(), Vec::new());
    let value = *back!(Backtrack::from_result(ctx.find_value(&var), span));
    Backtrack::Ok(value)
}

// - Unary expression

fn eval_un_exp<I: Interface, E: Extern>(
    runner: &mut RunnerContext<'_, Al, I, E>,
    ctx: &Context<'_>,
    op: &ast::UnOp,
    exp_inner: &ast::Exp,
    span: &Span,
) -> Backtrack<Value> {
    let value = back!(eval_exp(runner, ctx, exp_inner));
    let value = match op {
        ast::UnOp::Bool(boolean::UnOp::Not) => {
            let result = !back!(Backtrack::from_result(
                get::bool(runner.arena(), &value),
                span
            ));
            back!(Backtrack::from_result(
                make::bool(runner.arena_mut(), result, Span::default()),
                span
            ))
        }
        ast::UnOp::Num(op) => {
            let num = back!(Backtrack::from_result(
                get::num(runner.arena(), &value),
                span
            ));
            let result = num::un(*op, num);
            back!(Backtrack::from_result(
                make::num(runner.arena_mut(), result, Span::default()),
                &crate::lang::common::source::Span::default()
            ))
        }
    };
    Backtrack::Ok(value)
}

// - Binary expression

fn eval_bin_exp<I: Interface, E: Extern>(
    runner: &mut RunnerContext<'_, Al, I, E>,
    ctx: &Context<'_>,
    op: &ast::BinOp,
    exp_l: &ast::Exp,
    exp_r: &ast::Exp,
    span: &Span,
) -> Backtrack<Value> {
    let value_l = back!(eval_exp(runner, ctx, exp_l));
    let value_r = back!(eval_exp(runner, ctx, exp_r));
    let value = match op {
        ast::BinOp::Bool(op) => {
            let bool_l = back!(Backtrack::from_result(
                get::bool(runner.arena(), &value_l),
                span
            ));
            let bool_r = back!(Backtrack::from_result(
                get::bool(runner.arena(), &value_r),
                span
            ));
            let result = match op {
                boolean::BinOp::And => bool_l && bool_r,
                boolean::BinOp::Or => bool_l || bool_r,
                boolean::BinOp::Impl => !bool_l || bool_r,
                boolean::BinOp::Equiv => bool_l == bool_r,
            };
            back!(Backtrack::from_result(
                make::bool(runner.arena_mut(), result, Span::default()),
                &crate::lang::common::source::Span::default()
            ))
        }
        ast::BinOp::Num(op) => {
            let num_l = back!(Backtrack::from_result(
                get::num(runner.arena(), &value_l),
                span
            ));
            let num_r = back!(Backtrack::from_result(
                get::num(runner.arena(), &value_r),
                span
            ));
            let num = back!(Backtrack::from_result(num::bin(*op, num_l, num_r), span));
            back!(Backtrack::from_result(
                make::num(runner.arena_mut(), num, Span::default()),
                &crate::lang::common::source::Span::default()
            ))
        }
    };
    Backtrack::Ok(value)
}

// - Comparison expression

fn eval_cmp_exp<I: Interface, E: Extern>(
    runner: &mut RunnerContext<'_, Al, I, E>,
    ctx: &Context<'_>,
    op: &ast::CmpOp,
    exp_l: &ast::Exp,
    exp_r: &ast::Exp,
    span: &Span,
) -> Backtrack<Value> {
    let value_l = back!(eval_exp(runner, ctx, exp_l));
    let value_r = back!(eval_exp(runner, ctx, exp_r));
    let result = match op {
        ast::CmpOp::Bool(boolean::CmpOp::Eq) => runner.arena().syntax_eq(&value_l, &value_r),
        ast::CmpOp::Bool(boolean::CmpOp::Ne) => !runner.arena().syntax_eq(&value_l, &value_r),
        ast::CmpOp::Num(op) => {
            let num_l = back!(Backtrack::from_result(
                get::num(runner.arena(), &value_l),
                span
            ));
            let num_r = back!(Backtrack::from_result(
                get::num(runner.arena(), &value_r),
                span
            ));
            back!(Backtrack::from_result(num::cmp(*op, num_l, num_r), span))
        }
    };
    let value = back!(Backtrack::from_result(
        make::bool(runner.arena_mut(), result, Span::default()),
        &crate::lang::common::source::Span::default()
    ));
    Backtrack::Ok(value)
}

// - Upcast expression

fn eval_upcast_exp<I: Interface, E: Extern>(
    runner: &mut RunnerContext<'_, Al, I, E>,
    ctx: &Context<'_>,
    typ: &ast::Typ,
    exp_inner: &ast::Exp,
) -> Backtrack<Value> {
    let value = back!(eval_exp(runner, ctx, exp_inner));
    ops::cast_up(runner.arena_mut(), ctx, typ, value)
}

// - Downcast expression

fn eval_downcast_exp<I: Interface, E: Extern>(
    runner: &mut RunnerContext<'_, Al, I, E>,
    ctx: &Context<'_>,
    typ: &ast::Typ,
    exp_inner: &ast::Exp,
) -> Backtrack<Value> {
    let value = back!(eval_exp(runner, ctx, exp_inner));
    ops::cast_down(runner.arena_mut(), ctx, typ, value)
}

// - Subtype check expression

fn eval_sub_exp<I: Interface, E: Extern>(
    runner: &mut RunnerContext<'_, Al, I, E>,
    ctx: &Context<'_>,
    exp_inner: &ast::Exp,
    subcheck: &ast::Subcheck,
    span: &Span,
) -> Backtrack<Value> {
    let value = back!(eval_exp(runner, ctx, exp_inner));
    let tdenv = ctx.tdenv();
    let find_func = |name: &str| {
        let id = crate::phrase!(node: name.to_owned(), span: span.clone());
        ctx.find_func_typ(&id).ok()
    };
    let matches = back!(Backtrack::from_result(
        value::check(runner.arena(), &tdenv, &find_func, subcheck, &value),
        span
    ));
    let value = back!(Backtrack::from_result(
        make::bool(runner.arena_mut(), matches, Span::default()),
        &crate::lang::common::source::Span::default()
    ));
    Backtrack::Ok(value)
}

// - Match expression

fn eval_match_exp<I: Interface, E: Extern>(
    runner: &mut RunnerContext<'_, Al, I, E>,
    ctx: &Context<'_>,
    exp_inner: &ast::Exp,
    pattern: &ast::Pattern,
) -> Backtrack<Value> {
    let value = back!(eval_exp(runner, ctx, exp_inner));
    let matches = match (pattern, runner.arena().kind(&value)) {
        (ast::Pattern::Case(mixop), ValueKind::Case(value)) => value.eq_shape(mixop.as_ref()),
        (ast::Pattern::List(pattern), ValueKind::List(values)) => match pattern {
            ListPattern::Cons => !values.is_empty(),
            ListPattern::Fixed(len) => i64::try_from(values.len()).ok() == Some(*len),
            ListPattern::Nil => values.is_empty(),
        },
        (ast::Pattern::Opt(OptPattern::Some), ValueKind::Opt(Some(_)))
        | (ast::Pattern::Opt(OptPattern::None), ValueKind::Opt(None)) => true,
        _ => false,
    };
    let value = back!(Backtrack::from_result(
        make::bool(runner.arena_mut(), matches, Span::default()),
        &crate::lang::common::source::Span::default()
    ));
    Backtrack::Ok(value)
}

// - Tuple expression

fn eval_tuple_exp<I: Interface, E: Extern>(
    runner: &mut RunnerContext<'_, Al, I, E>,
    ctx: &Context<'_>,
    exps: &[ast::Exp],
    typ: &ast::Typ,
) -> Backtrack<Value> {
    let values = back!(eval_exps(runner, ctx, exps));
    let value = back!(Backtrack::from_result(
        make::tuple(runner.arena_mut(), typ, values, Span::default()),
        &crate::lang::common::source::Span::default()
    ));
    Backtrack::Ok(value)
}

// - Case expression

fn eval_case_exp<I: Interface, E: Extern>(
    runner: &mut RunnerContext<'_, Al, I, E>,
    ctx: &Context<'_>,
    not_exp: &ast::NotExp,
    typ: &ast::Typ,
    span: &Span,
) -> Backtrack<Value> {
    let mut values = Vec::new();
    for exp in not_exp.args() {
        values.push(back!(eval_exp(runner, ctx, exp)));
    }
    let case = back!(Backtrack::from_result(
        ast::Mixop::fill(&not_exp.to_mixop(), values),
        span
    ));
    let value = back!(Backtrack::from_result(
        make::case_(runner.arena_mut(), typ, case, Span::default()),
        &crate::lang::common::source::Span::default()
    ));
    Backtrack::Ok(value)
}

// - Struct expression

fn eval_str_exp<I: Interface, E: Extern>(
    runner: &mut RunnerContext<'_, Al, I, E>,
    ctx: &Context<'_>,
    exp_fields: &[ast::ExpField],
    typ: &ast::Typ,
) -> Backtrack<Value> {
    let mut value_fields = Vec::with_capacity(exp_fields.len());
    for (atom, exp) in exp_fields {
        value_fields.push((atom.clone(), back!(eval_exp(runner, ctx, exp))));
    }
    let value = back!(Backtrack::from_result(
        make::structure(runner.arena_mut(), typ, value_fields, Span::default()),
        &crate::lang::common::source::Span::default()
    ));
    Backtrack::Ok(value)
}

// - Optional expression

fn eval_opt_exp<I: Interface, E: Extern>(
    runner: &mut RunnerContext<'_, Al, I, E>,
    ctx: &Context<'_>,
    exp: &Option<Box<ast::Exp>>,
    typ: &ast::Typ,
) -> Backtrack<Value> {
    let value = match exp {
        Some(exp) => Some(back!(eval_exp(runner, ctx, exp))),
        None => None,
    };
    let value = back!(Backtrack::from_result(
        make::opt(runner.arena_mut(), typ, value, Span::default()),
        &crate::lang::common::source::Span::default()
    ));
    Backtrack::Ok(value)
}

// - List expression

fn eval_list_exp<I: Interface, E: Extern>(
    runner: &mut RunnerContext<'_, Al, I, E>,
    ctx: &Context<'_>,
    exps: &[ast::Exp],
    typ: &ast::Typ,
) -> Backtrack<Value> {
    let values = back!(eval_exps(runner, ctx, exps));
    let value = back!(Backtrack::from_result(
        make::list(runner.arena_mut(), typ, values, Span::default()),
        &crate::lang::common::source::Span::default()
    ));
    Backtrack::Ok(value)
}

// - Cons expression

fn eval_cons_exp<I: Interface, E: Extern>(
    runner: &mut RunnerContext<'_, Al, I, E>,
    ctx: &Context<'_>,
    exp_head: &ast::Exp,
    exp_tail: &ast::Exp,
    typ: &ast::Typ,
    span: &Span,
) -> Backtrack<Value> {
    let value_head = back!(eval_exp(runner, ctx, exp_head));
    let value_tail = back!(eval_exp(runner, ctx, exp_tail));
    let values_tail = back!(Backtrack::from_result(
        get::list(runner.arena(), &value_tail),
        span
    ));
    let mut values = Vec::with_capacity(values_tail.len() + 1);
    values.push(value_head);
    values.extend_from_slice(values_tail);
    let value = back!(Backtrack::from_result(
        make::list(runner.arena_mut(), typ, values, Span::default()),
        &crate::lang::common::source::Span::default()
    ));
    Backtrack::Ok(value)
}

// - Concatenation expression

fn eval_cat_exp<I: Interface, E: Extern>(
    runner: &mut RunnerContext<'_, Al, I, E>,
    ctx: &Context<'_>,
    exp_l: &ast::Exp,
    exp_r: &ast::Exp,
    typ: &ast::Typ,
) -> Backtrack<Value> {
    let value_l = back!(eval_exp(runner, ctx, exp_l));
    let value_r = back!(eval_exp(runner, ctx, exp_r));
    let value = match (runner.arena().kind(&value_l), runner.arena().kind(&value_r)) {
        (ValueKind::Text(text_l), ValueKind::Text(text_r)) => {
            let text = format!("{text_l}{text_r}");
            back!(Backtrack::from_result(
                make::text(runner.arena_mut(), text, Span::default()),
                &crate::lang::common::source::Span::default()
            ))
        }
        (ValueKind::List(values_l), ValueKind::List(values_r)) => {
            let mut values = values_l.clone();
            values.extend_from_slice(values_r);
            back!(Backtrack::from_result(
                make::list(runner.arena_mut(), typ, values, Span::default()),
                &crate::lang::common::source::Span::default()
            ))
        }
        _ => {
            return Backtrack::err(
                Span::over(&[exp_l.span.clone(), exp_r.span.clone()]),
                ErrorKind::Expr(ExprErrorKind::ConcatenationOperandMismatch),
            );
        }
    };
    Backtrack::Ok(value)
}

// - Membership expression

fn eval_mem_exp<I: Interface, E: Extern>(
    runner: &mut RunnerContext<'_, Al, I, E>,
    ctx: &Context<'_>,
    exp_elem: &ast::Exp,
    exp_list: &ast::Exp,
    span: &Span,
) -> Backtrack<Value> {
    let value_elem = back!(eval_exp(runner, ctx, exp_elem));
    let value_list = back!(eval_exp(runner, ctx, exp_list));
    let values = back!(Backtrack::from_result(
        get::list(runner.arena(), &value_list),
        span
    ));
    let result = values
        .iter()
        .any(|value| runner.arena().syntax_eq(value, &value_elem));
    let value = back!(Backtrack::from_result(
        make::bool(runner.arena_mut(), result, Span::default()),
        span
    ));
    Backtrack::Ok(value)
}

// - Length expression

fn eval_len_exp<I: Interface, E: Extern>(
    runner: &mut RunnerContext<'_, Al, I, E>,
    ctx: &Context<'_>,
    exp_inner: &ast::Exp,
) -> Backtrack<Value> {
    let value = back!(eval_exp(runner, ctx, exp_inner));
    let len = match runner.arena().kind(&value) {
        ValueKind::Text(text) => text.len(),
        ValueKind::List(values) => values.len(),
        _ => {
            return Backtrack::err(
                exp_inner.span.clone(),
                ErrorKind::Expr(ExprErrorKind::LengthOperandMismatch),
            );
        }
    };
    let value = back!(Backtrack::from_result(
        make::nat(runner.arena_mut(), (len as u64).into(), Span::default()),
        &crate::lang::common::source::Span::default()
    ));
    Backtrack::Ok(value)
}

// - Field access expression

fn eval_dot_exp<I: Interface, E: Extern>(
    runner: &mut RunnerContext<'_, Al, I, E>,
    ctx: &Context<'_>,
    exp_base: &ast::Exp,
    atom: &ast::Atom,
    span: &Span,
) -> Backtrack<Value> {
    let value = back!(eval_exp(runner, ctx, exp_base));
    ops::access_dot(runner.arena_mut(), &value, atom, span)
}

// - Index expression

fn eval_idx_exp<I: Interface, E: Extern>(
    runner: &mut RunnerContext<'_, Al, I, E>,
    ctx: &Context<'_>,
    exp_base: &ast::Exp,
    exp_idx: &ast::Exp,
) -> Backtrack<Value> {
    let value = back!(eval_exp(runner, ctx, exp_base));
    let value_idx = back!(eval_exp(runner, ctx, exp_idx));
    ops::access_index(
        runner.arena_mut(),
        &value,
        &value_idx,
        &exp_base.span,
        &exp_idx.span,
    )
}

// - Slice expression

fn eval_slice_exp<I: Interface, E: Extern>(
    runner: &mut RunnerContext<'_, Al, I, E>,
    ctx: &Context<'_>,
    exp_base: &ast::Exp,
    exp_idx: &ast::Exp,
    exp_len: &ast::Exp,
    typ: &ast::Typ,
) -> Backtrack<Value> {
    let value = back!(eval_exp(runner, ctx, exp_base));
    let value_idx = back!(eval_exp(runner, ctx, exp_idx));
    let value_len = back!(eval_exp(runner, ctx, exp_len));
    let span_bounds = if matches!(runner.arena().kind(&value), ValueKind::Text(_)) {
        &exp_idx.span
    } else {
        &exp_len.span
    };
    ops::access_slice(
        runner.arena_mut(),
        &value,
        &value_idx,
        &value_len,
        typ,
        &exp_base.span,
        &exp_idx.span,
        &exp_len.span,
        span_bounds,
    )
}

// - Update expression

fn eval_upd_exp<I: Interface, E: Extern>(
    runner: &mut RunnerContext<'_, Al, I, E>,
    ctx: &Context<'_>,
    exp_base: &ast::Exp,
    path: &ast::Path,
    exp_upd: &ast::Exp,
) -> Backtrack<Value> {
    let value_base = back!(eval_exp(runner, ctx, exp_base));
    let value_upd = back!(eval_exp(runner, ctx, exp_upd));
    eval_update_path(runner, ctx, &value_base, path, value_upd)
}

// - Call expression

fn eval_call_exp<I: Interface, E: Extern>(
    runner: &mut RunnerContext<'_, Al, I, E>,
    ctx: &Context<'_>,
    id: &ast::Id,
    targs: &[ast::Typ],
    args: &[ast::Arg],
) -> Backtrack<Value> {
    let theta = ctx.theta_local();
    let mut targs_subst = Vec::with_capacity(targs.len());
    for targ in targs {
        targs_subst.push(back!(Backtrack::from_result(
            subst_typ(&theta, targ),
            &targ.span
        )));
    }
    let values = back!(eval_args(runner, ctx, args));
    invoke_func(runner, ctx, id, &targs_subst, &values)
}

// - Iteration expression

fn eval_iter_exp<I: Interface, E: Extern>(
    runner: &mut RunnerContext<'_, Al, I, E>,
    ctx: &Context<'_>,
    exp: &ast::Exp,
    exp_inner: &ast::Exp,
    iter: &ast::Iter,
    vars: &[ast::Var],
    typ: &ast::Typ,
) -> Backtrack<Value> {
    let span = &exp.span;
    if let Some(var) = is_iter_var_exp(exp) {
        return Backtrack::Ok(*back!(Backtrack::from_result(ctx.find_value(&var), span)));
    }
    let value = match iter {
        ast::Iter::Opt => {
            let ctx_sub = back!(Backtrack::from_result(
                ctx.sub_opt(runner.arena(), vars),
                span
            ));
            let value = match ctx_sub {
                Some(ctx_sub) => Some(back!(eval_exp(runner, &ctx_sub, exp_inner))),
                None => None,
            };
            back!(Backtrack::from_result(
                make::opt(runner.arena_mut(), typ, value, Span::default()),
                &crate::lang::common::source::Span::default()
            ))
        }
        ast::Iter::List => {
            let ctxs_sub = back!(Backtrack::from_result(
                ctx.sub_list(runner.arena(), vars),
                span
            ));
            let mut values = Vec::with_capacity(ctxs_sub.len());
            for ctx_sub in ctxs_sub {
                values.push(back!(eval_exp(runner, &ctx_sub, exp_inner)));
            }
            back!(Backtrack::from_result(
                make::list(runner.arena_mut(), typ, values, Span::default()),
                &crate::lang::common::source::Span::default()
            ))
        }
    };
    Backtrack::Ok(value)
}
