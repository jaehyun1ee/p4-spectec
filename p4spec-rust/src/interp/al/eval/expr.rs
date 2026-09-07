//! AL expression evaluation

use crate::interp::al::error::ExprErrorKind;
use std::rc::Rc;

use crate::{
    lang::{
        al::ast,
        common::{Variable, source::Span},
        data::value::{Value, ValueKind, get, make},
        il::ast::{ListPattern, OptPattern},
        traits::eq::SyntaxEq,
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
use super::{call::invoke_func, ops};

// = Expression evaluation

pub(super) fn eval_exp<I: Interface, E: Extern>(
    runner: &mut RunnerContext<'_, Al, I, E>,
    ctx: &Context<'_>,
    exp: &ast::Exp,
) -> Backtrack<Rc<Value>> {
    let span = &exp.span;
    let typ = crate::phrase!(node: exp.note.as_ref().clone(), span: span.clone());
    match &exp.node {
        ast::ExpKind::Bool(value) => Backtrack::Ok(make::bool(*value, Span::default())),
        ast::ExpKind::Num(value) => Backtrack::Ok(make::num(value.clone(), Span::default())),
        ast::ExpKind::Text(value) => Backtrack::Ok(make::text(value.clone(), Span::default())),
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
) -> Backtrack<Vec<Rc<Value>>> {
    let mut values = Vec::with_capacity(exps.len());
    for exp in exps {
        values.push(back!(eval_exp(runner, ctx, exp)));
    }
    Backtrack::Ok(values)
}

// - Variable expression

fn eval_var_exp(ctx: &Context<'_>, id: &ast::Id, span: &Span) -> Backtrack<Rc<Value>> {
    let var = Variable::new(id.clone(), Vec::new());
    let value = back!(Backtrack::from_result(ctx.find_value(&var), span)).clone();
    Backtrack::Ok(value)
}

// - Unary expression

fn eval_un_exp<I: Interface, E: Extern>(
    runner: &mut RunnerContext<'_, Al, I, E>,
    ctx: &Context<'_>,
    op: &ast::UnOp,
    exp_inner: &ast::Exp,
    span: &Span,
) -> Backtrack<Rc<Value>> {
    let value = back!(eval_exp(runner, ctx, exp_inner));
    let value = match op {
        ast::UnOp::Bool(boolean::UnOp::Not) => make::bool(
            !back!(Backtrack::from_result(get::bool(&value), span)),
            Span::default(),
        ),
        ast::UnOp::Num(op) => {
            let num = back!(Backtrack::from_result(get::num(&value), span));
            make::num(num::un(*op, num), Span::default())
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
) -> Backtrack<Rc<Value>> {
    let value_l = back!(eval_exp(runner, ctx, exp_l));
    let value_r = back!(eval_exp(runner, ctx, exp_r));
    let value = match op {
        ast::BinOp::Bool(op) => {
            let bool_l = back!(Backtrack::from_result(get::bool(&value_l), span));
            let bool_r = back!(Backtrack::from_result(get::bool(&value_r), span));
            let result = match op {
                boolean::BinOp::And => bool_l && bool_r,
                boolean::BinOp::Or => bool_l || bool_r,
                boolean::BinOp::Impl => !bool_l || bool_r,
                boolean::BinOp::Equiv => bool_l == bool_r,
            };
            make::bool(result, Span::default())
        }
        ast::BinOp::Num(op) => {
            let num_l = back!(Backtrack::from_result(get::num(&value_l), span));
            let num_r = back!(Backtrack::from_result(get::num(&value_r), span));
            let num = back!(Backtrack::from_result(num::bin(*op, num_l, num_r), span));
            make::num(num, Span::default())
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
) -> Backtrack<Rc<Value>> {
    let value_l = back!(eval_exp(runner, ctx, exp_l));
    let value_r = back!(eval_exp(runner, ctx, exp_r));
    let result = match op {
        ast::CmpOp::Bool(boolean::CmpOp::Eq) => value_l.syntax_eq(&value_r),
        ast::CmpOp::Bool(boolean::CmpOp::Ne) => !value_l.syntax_eq(&value_r),
        ast::CmpOp::Num(op) => {
            let num_l = back!(Backtrack::from_result(get::num(&value_l), span));
            let num_r = back!(Backtrack::from_result(get::num(&value_r), span));
            back!(Backtrack::from_result(num::cmp(*op, num_l, num_r), span))
        }
    };
    let value = make::bool(result, Span::default());
    Backtrack::Ok(value)
}

// - Upcast expression

fn eval_upcast_exp<I: Interface, E: Extern>(
    runner: &mut RunnerContext<'_, Al, I, E>,
    ctx: &Context<'_>,
    typ: &ast::Typ,
    exp_inner: &ast::Exp,
) -> Backtrack<Rc<Value>> {
    let value = back!(eval_exp(runner, ctx, exp_inner));
    ops::cast_up(ctx, typ, value)
}

// - Downcast expression

fn eval_downcast_exp<I: Interface, E: Extern>(
    runner: &mut RunnerContext<'_, Al, I, E>,
    ctx: &Context<'_>,
    typ: &ast::Typ,
    exp_inner: &ast::Exp,
) -> Backtrack<Rc<Value>> {
    let value = back!(eval_exp(runner, ctx, exp_inner));
    ops::cast_down(ctx, typ, value)
}

// - Subtype check expression

fn eval_sub_exp<I: Interface, E: Extern>(
    runner: &mut RunnerContext<'_, Al, I, E>,
    ctx: &Context<'_>,
    exp_inner: &ast::Exp,
    subcheck: &ast::Subcheck,
    span: &Span,
) -> Backtrack<Rc<Value>> {
    let value = back!(eval_exp(runner, ctx, exp_inner));
    let tdenv = ctx.type_env();
    let find_func = |name: &str| {
        let id = crate::phrase!(node: name.to_owned(), span: span.clone());
        ctx.find_func_typ(&id).ok()
    };
    let matches = back!(Backtrack::from_result(
        value::check(&tdenv, &find_func, subcheck, &value),
        span
    ));
    let value = make::bool(matches, Span::default());
    Backtrack::Ok(value)
}

// - Match expression

fn eval_match_exp<I: Interface, E: Extern>(
    runner: &mut RunnerContext<'_, Al, I, E>,
    ctx: &Context<'_>,
    exp_inner: &ast::Exp,
    pattern: &ast::Pattern,
) -> Backtrack<Rc<Value>> {
    let value = back!(eval_exp(runner, ctx, exp_inner));
    let matches = match (pattern, &value.node) {
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
    let value = make::bool(matches, Span::default());
    Backtrack::Ok(value)
}

// - Tuple expression

fn eval_tuple_exp<I: Interface, E: Extern>(
    runner: &mut RunnerContext<'_, Al, I, E>,
    ctx: &Context<'_>,
    exps: &[ast::Exp],
    typ: &ast::Typ,
) -> Backtrack<Rc<Value>> {
    let values = back!(eval_exps(runner, ctx, exps));
    let value = make::tuple(typ, values, Span::default());
    Backtrack::Ok(value)
}

// - Case expression

fn eval_case_exp<I: Interface, E: Extern>(
    runner: &mut RunnerContext<'_, Al, I, E>,
    ctx: &Context<'_>,
    not_exp: &ast::NotExp,
    typ: &ast::Typ,
    span: &Span,
) -> Backtrack<Rc<Value>> {
    let mut values = Vec::new();
    for exp in not_exp.args() {
        values.push(back!(eval_exp(runner, ctx, exp)));
    }
    let case = back!(Backtrack::from_result(
        ast::Mixop::fill(&not_exp.to_mixop(), values),
        span
    ));
    let value = make::case_(typ, case, Span::default());
    Backtrack::Ok(value)
}

// - Struct expression

fn eval_str_exp<I: Interface, E: Extern>(
    runner: &mut RunnerContext<'_, Al, I, E>,
    ctx: &Context<'_>,
    exp_fields: &[ast::ExpField],
    typ: &ast::Typ,
) -> Backtrack<Rc<Value>> {
    let mut value_fields = Vec::with_capacity(exp_fields.len());
    for (atom, exp) in exp_fields {
        value_fields.push((atom.clone(), back!(eval_exp(runner, ctx, exp))));
    }
    let value = make::structure(typ, value_fields, Span::default());
    Backtrack::Ok(value)
}

// - Optional expression

fn eval_opt_exp<I: Interface, E: Extern>(
    runner: &mut RunnerContext<'_, Al, I, E>,
    ctx: &Context<'_>,
    exp: &Option<Box<ast::Exp>>,
    typ: &ast::Typ,
) -> Backtrack<Rc<Value>> {
    let value = match exp {
        Some(exp) => Some(back!(eval_exp(runner, ctx, exp))),
        None => None,
    };
    let value = make::opt(typ, value, Span::default());
    Backtrack::Ok(value)
}

// - List expression

fn eval_list_exp<I: Interface, E: Extern>(
    runner: &mut RunnerContext<'_, Al, I, E>,
    ctx: &Context<'_>,
    exps: &[ast::Exp],
    typ: &ast::Typ,
) -> Backtrack<Rc<Value>> {
    let values = back!(eval_exps(runner, ctx, exps));
    let value = make::list(typ, values, Span::default());
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
) -> Backtrack<Rc<Value>> {
    let value_head = back!(eval_exp(runner, ctx, exp_head));
    let value_tail = back!(eval_exp(runner, ctx, exp_tail));
    let values_tail = back!(Backtrack::from_result(get::list(&value_tail), span));
    let mut values = Vec::with_capacity(values_tail.len() + 1);
    values.push(value_head);
    values.extend_from_slice(values_tail);
    let value = make::list(typ, values, Span::default());
    Backtrack::Ok(value)
}

// - Concatenation expression

fn eval_cat_exp<I: Interface, E: Extern>(
    runner: &mut RunnerContext<'_, Al, I, E>,
    ctx: &Context<'_>,
    exp_l: &ast::Exp,
    exp_r: &ast::Exp,
    typ: &ast::Typ,
) -> Backtrack<Rc<Value>> {
    let value_l = back!(eval_exp(runner, ctx, exp_l));
    let value_r = back!(eval_exp(runner, ctx, exp_r));
    let value = match (&value_l.node, &value_r.node) {
        (ValueKind::Text(text_l), ValueKind::Text(text_r)) => {
            make::text(format!("{text_l}{text_r}"), Span::default())
        }
        (ValueKind::List(values_l), ValueKind::List(values_r)) => {
            let mut values = values_l.clone();
            values.extend_from_slice(values_r);
            make::list(typ, values, Span::default())
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
) -> Backtrack<Rc<Value>> {
    let value_elem = back!(eval_exp(runner, ctx, exp_elem));
    let value_list = back!(eval_exp(runner, ctx, exp_list));
    let values = back!(Backtrack::from_result(get::list(&value_list), span));
    let value = make::bool(
        values.iter().any(|value| value.syntax_eq(&value_elem)),
        Span::default(),
    );
    Backtrack::Ok(value)
}

// - Length expression

fn eval_len_exp<I: Interface, E: Extern>(
    runner: &mut RunnerContext<'_, Al, I, E>,
    ctx: &Context<'_>,
    exp_inner: &ast::Exp,
) -> Backtrack<Rc<Value>> {
    let value = back!(eval_exp(runner, ctx, exp_inner));
    let len = match &value.node {
        ValueKind::Text(text) => text.len(),
        ValueKind::List(values) => values.len(),
        _ => {
            return Backtrack::err(
                exp_inner.span.clone(),
                ErrorKind::Expr(ExprErrorKind::LengthOperandMismatch),
            );
        }
    };
    let value = make::nat((len as u64).into(), Span::default());
    Backtrack::Ok(value)
}

// - Field access expression

fn eval_dot_exp<I: Interface, E: Extern>(
    runner: &mut RunnerContext<'_, Al, I, E>,
    ctx: &Context<'_>,
    exp_base: &ast::Exp,
    atom: &ast::Atom,
    span: &Span,
) -> Backtrack<Rc<Value>> {
    let value = back!(eval_exp(runner, ctx, exp_base));
    ops::access_dot(&value, atom, span)
}

// - Index expression

fn eval_idx_exp<I: Interface, E: Extern>(
    runner: &mut RunnerContext<'_, Al, I, E>,
    ctx: &Context<'_>,
    exp_base: &ast::Exp,
    exp_idx: &ast::Exp,
) -> Backtrack<Rc<Value>> {
    let value = back!(eval_exp(runner, ctx, exp_base));
    let value_idx = back!(eval_exp(runner, ctx, exp_idx));
    ops::access_index(&value, &value_idx, &exp_base.span, &exp_idx.span)
}

// - Slice expression

fn eval_slice_exp<I: Interface, E: Extern>(
    runner: &mut RunnerContext<'_, Al, I, E>,
    ctx: &Context<'_>,
    exp_base: &ast::Exp,
    exp_idx: &ast::Exp,
    exp_len: &ast::Exp,
    typ: &ast::Typ,
) -> Backtrack<Rc<Value>> {
    let value = back!(eval_exp(runner, ctx, exp_base));
    let value_idx = back!(eval_exp(runner, ctx, exp_idx));
    let value_len = back!(eval_exp(runner, ctx, exp_len));
    let span_bounds = if matches!(value.node, ValueKind::Text(_)) {
        &exp_idx.span
    } else {
        &exp_len.span
    };
    ops::access_slice(
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
) -> Backtrack<Rc<Value>> {
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
) -> Backtrack<Rc<Value>> {
    let theta = ctx.local_theta();
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
) -> Backtrack<Rc<Value>> {
    let span = &exp.span;
    if let Some(var) = is_iter_var_exp(exp) {
        return Backtrack::Ok(back!(Backtrack::from_result(ctx.find_value(&var), span)).clone());
    }
    let value = match iter {
        ast::Iter::Opt => {
            let ctx_sub = back!(Backtrack::from_result(ctx.sub_opt(vars), span));
            let value = match ctx_sub {
                Some(ctx_sub) => Some(back!(eval_exp(runner, &ctx_sub, exp_inner))),
                None => None,
            };
            make::opt(typ, value, Span::default())
        }
        ast::Iter::List => {
            let ctxs_sub = back!(Backtrack::from_result(ctx.sub_list(vars), span));
            let mut values = Vec::with_capacity(ctxs_sub.len());
            for ctx_sub in ctxs_sub {
                values.push(back!(eval_exp(runner, &ctx_sub, exp_inner)));
            }
            make::list(typ, values, Span::default())
        }
    };
    Backtrack::Ok(value)
}

// = Argument evaluation

fn eval_arg<I: Interface, E: Extern>(
    runner: &mut RunnerContext<'_, Al, I, E>,
    ctx: &Context<'_>,
    arg: &ast::Arg,
) -> Backtrack<Rc<Value>> {
    match &arg.node {
        ast::ArgKind::Exp(exp) => eval_exp(runner, ctx, exp),
        ast::ArgKind::Def(id) => eval_def_arg(ctx, id, &arg.span),
    }
}

pub(super) fn eval_args<I: Interface, E: Extern>(
    runner: &mut RunnerContext<'_, Al, I, E>,
    ctx: &Context<'_>,
    args: &[ast::Arg],
) -> Backtrack<Vec<Rc<Value>>> {
    let mut values = Vec::with_capacity(args.len());
    for arg in args {
        values.push(back!(eval_arg(runner, ctx, arg)));
    }
    Backtrack::Ok(values)
}

// - Function argument

fn eval_def_arg(ctx: &Context<'_>, id: &ast::Id, span: &Span) -> Backtrack<Rc<Value>> {
    let typ_func = back!(Backtrack::from_result(ctx.find_func_typ(id), span));
    let value = make::func(
        id.clone(),
        typ_func.tparams,
        typ_func.typs_params,
        *typ_func.typ_ret,
        Span::default(),
    );
    Backtrack::Ok(value)
}

// = Path evaluation

// - Access

fn eval_access_path<I: Interface, E: Extern>(
    runner: &mut RunnerContext<'_, Al, I, E>,
    ctx: &Context<'_>,
    value_base: &Rc<Value>,
    path: &ast::Path,
) -> Backtrack<Rc<Value>> {
    match &path.node {
        ast::PathKind::Root => Backtrack::Ok(value_base.clone()),
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

fn eval_access_idx_path<I: Interface, E: Extern>(
    runner: &mut RunnerContext<'_, Al, I, E>,
    ctx: &Context<'_>,
    value_base: &Rc<Value>,
    path: &ast::Path,
    exp_idx: &ast::Exp,
) -> Backtrack<Rc<Value>> {
    let value = back!(eval_access_path(runner, ctx, value_base, path));
    let value_idx = back!(eval_exp(runner, ctx, exp_idx));
    ops::access_index(&value, &value_idx, &path.span, &exp_idx.span)
}

// - Slice access path

fn eval_access_slice_path<I: Interface, E: Extern>(
    runner: &mut RunnerContext<'_, Al, I, E>,
    ctx: &Context<'_>,
    value_base: &Rc<Value>,
    path: &ast::Path,
    exp_idx: &ast::Exp,
    exp_len: &ast::Exp,
) -> Backtrack<Rc<Value>> {
    let typ = crate::phrase!(node: path.note.as_ref().clone(), span: path.span.clone());
    let value = back!(eval_access_path(runner, ctx, value_base, path));
    let value_idx = back!(eval_exp(runner, ctx, exp_idx));
    let value_len = back!(eval_exp(runner, ctx, exp_len));
    ops::access_slice(
        &value,
        &value_idx,
        &value_len,
        &typ,
        &path.span,
        &exp_idx.span,
        &exp_len.span,
        &exp_len.span,
    )
}

// - Field access path

fn eval_access_dot_path<I: Interface, E: Extern>(
    runner: &mut RunnerContext<'_, Al, I, E>,
    ctx: &Context<'_>,
    value_base: &Rc<Value>,
    path: &ast::Path,
    atom: &ast::Atom,
) -> Backtrack<Rc<Value>> {
    let value = back!(eval_access_path(runner, ctx, value_base, path));
    ops::access_dot(&value, atom, &path.span)
}

// - Update

fn eval_update_path<I: Interface, E: Extern>(
    runner: &mut RunnerContext<'_, Al, I, E>,
    ctx: &Context<'_>,
    value_base: &Rc<Value>,
    path: &ast::Path,
    value_upd: Rc<Value>,
) -> Backtrack<Rc<Value>> {
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

fn eval_update_idx_path<I: Interface, E: Extern>(
    runner: &mut RunnerContext<'_, Al, I, E>,
    ctx: &Context<'_>,
    value_base: &Rc<Value>,
    path: &ast::Path,
    exp_idx: &ast::Exp,
    value_upd: Rc<Value>,
) -> Backtrack<Rc<Value>> {
    let typ = crate::phrase!(node: path.note.as_ref().clone(), span: path.span.clone());
    let value = back!(eval_access_path(runner, ctx, value_base, path));
    let value_idx = back!(eval_exp(runner, ctx, exp_idx));
    let value = back!(ops::update_index(
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

fn eval_update_slice_path<I: Interface, E: Extern>(
    runner: &mut RunnerContext<'_, Al, I, E>,
    ctx: &Context<'_>,
    value_base: &Rc<Value>,
    path: &ast::Path,
    exp_idx: &ast::Exp,
    exp_len: &ast::Exp,
    value_upd: Rc<Value>,
) -> Backtrack<Rc<Value>> {
    let typ = crate::phrase!(node: path.note.as_ref().clone(), span: path.span.clone());
    let value = back!(eval_access_path(runner, ctx, value_base, path));
    let value_idx = back!(eval_exp(runner, ctx, exp_idx));
    let value_len = back!(eval_exp(runner, ctx, exp_len));
    let value = back!(ops::update_slice(
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

fn eval_update_dot_path<I: Interface, E: Extern>(
    runner: &mut RunnerContext<'_, Al, I, E>,
    ctx: &Context<'_>,
    value_base: &Rc<Value>,
    path: &ast::Path,
    atom: &ast::Atom,
    value_upd: Rc<Value>,
) -> Backtrack<Rc<Value>> {
    let typ = crate::phrase!(node: path.note.as_ref().clone(), span: path.span.clone());
    let value = back!(eval_access_path(runner, ctx, value_base, path));
    let value_fields = back!(Backtrack::from_result(get::structure(&value), &path.span));
    let value_fields = value_fields
        .iter()
        .map(|(field, value)| {
            (
                field.clone(),
                if field.node == atom.node {
                    value_upd.clone()
                } else {
                    value.clone()
                },
            )
        })
        .collect();
    let value = make::structure(&typ, value_fields, Span::default());
    eval_update_path(runner, ctx, value_base, path, value)
}
