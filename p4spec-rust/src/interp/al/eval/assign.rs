//! Destructuring assignments preserve iteration paths and isolate list rows

use std::{borrow::Borrow, rc::Rc};

use crate::interp::al::error::AssignErrorKind;

use crate::{
    lang::{
        al::ast,
        common::{Variable, source::Span},
        data::{
            typ,
            value::{Value, ValueArena, ValueKind, get, make},
        },
        traits::print::Print,
    },
    phrase,
};

use super::super::{
    backtrack::{Backtrack, backtrack, backtrack_from_result},
    context::Context,
    error::ErrorKind,
    util::is_iter_var_exp,
};

// = Expression assignment

pub fn assign_exp<'global>(
    arena: &mut ValueArena,
    ctx: Context<'global>,
    exp: &ast::Exp,
    value: Value,
) -> Backtrack<Context<'global>> {
    match (&exp.node, arena.kind(&value)) {
        (ast::ExpKind::Var(id), _) => assign_var_exp(arena, ctx, id, value),
        (ast::ExpKind::Tuple(exps), ValueKind::Tuple(values)) => {
            let values = values.to_vec();
            assign_tuple_exp(arena, ctx, exps, &values)
        }
        (ast::ExpKind::Case(not_exp), ValueKind::Case(value_case)) => {
            let values = value_case.args().into_iter().copied().collect::<Vec<_>>();
            assign_case_exp(arena, ctx, not_exp, &values)
        }
        (ast::ExpKind::Str(exp_fields), ValueKind::Struct(value_fields)) => {
            let values = value_fields
                .iter()
                .map(|(_, value)| *value)
                .collect::<Vec<_>>();
            assign_str_exp(arena, ctx, exp_fields, &values)
        }
        (ast::ExpKind::Opt(exp_opt), ValueKind::Opt(value_opt)) => {
            let value_opt = *value_opt;
            assign_opt_exp(arena, ctx, exp, exp_opt, &value, &value_opt)
        }
        (ast::ExpKind::List(exps), ValueKind::List(values)) => {
            let values = values.to_vec();
            assign_list_exp(arena, ctx, exps, &values)
        }
        (ast::ExpKind::Cons(exp_head, exp_tail), ValueKind::List(values)) => {
            let values = values.to_vec();
            assign_cons_exp(arena, ctx, exp, exp_head, exp_tail, &value, &values)
        }
        (ast::ExpKind::Iter(exp_inner, (iter, vars)), _) => {
            assign_iter_exp(arena, ctx, exp, exp_inner, iter, vars, value)
        }
        _ => Backtrack::err(
            exp.span.clone(),
            ErrorKind::Assign(AssignErrorKind::Mismatch {
                expression: Print::to_string(exp),
                value: arena.to_string(&value),
            }),
        ),
    }
}

pub fn assign_exps<'global, T: Borrow<ast::Exp>>(
    arena: &mut ValueArena,
    mut ctx: Context<'global>,
    exps: &[T],
    values: &[Value],
) -> Backtrack<Context<'global>> {
    if exps.len() != values.len() {
        return Backtrack::err(
            Span::over(
                &exps
                    .iter()
                    .map(|exp| exp.borrow().span.clone())
                    .collect::<Vec<_>>(),
            ),
            ErrorKind::Assign(AssignErrorKind::ExpressionArityMismatch {
                expected: exps.len(),
                actual: values.len(),
            }),
        );
    }
    for (exp, value) in exps.iter().zip(values) {
        ctx = backtrack!(assign_exp(arena, ctx, exp.borrow(), *value));
    }
    Backtrack::Ok(ctx)
}

// - Variable expression

fn assign_var_exp<'global>(
    _arena: &mut ValueArena,
    mut ctx: Context<'global>,
    id: &ast::Id,
    value: Value,
) -> Backtrack<Context<'global>> {
    ctx.add_value(Variable::new(id.clone(), vec![]), value);
    Backtrack::Ok(ctx)
}

// - Tuple expression

fn assign_tuple_exp<'global>(
    arena: &mut ValueArena,
    ctx: Context<'global>,
    exps: &[ast::Exp],
    values: &[Value],
) -> Backtrack<Context<'global>> {
    assign_exps(arena, ctx, exps, values)
}

// - Case expression

fn assign_case_exp<'global>(
    arena: &mut ValueArena,
    ctx: Context<'global>,
    not_exp: &ast::NotExp,
    values: &[Value],
) -> Backtrack<Context<'global>> {
    let exps = not_exp.args();
    assign_exps(arena, ctx, &exps, values)
}

// - Struct expression

fn assign_str_exp<'global>(
    arena: &mut ValueArena,
    ctx: Context<'global>,
    exp_fields: &[ast::ExpField],
    values: &[Value],
) -> Backtrack<Context<'global>> {
    let exps = exp_fields.iter().map(|(_, exp)| exp).collect::<Vec<_>>();
    assign_exps(arena, ctx, &exps, values)
}

// - Optional expression

fn assign_opt_exp<'global>(
    arena: &mut ValueArena,
    ctx: Context<'global>,
    exp: &ast::Exp,
    exp_opt: &Option<Box<ast::Exp>>,
    value: &Value,
    value_opt: &Option<Value>,
) -> Backtrack<Context<'global>> {
    match (exp_opt, value_opt) {
        (Some(exp), Some(value)) => assign_exp(arena, ctx, exp, *value),
        (None, None) => Backtrack::Ok(ctx),
        _ => Backtrack::err(
            exp.span.clone(),
            ErrorKind::Assign(AssignErrorKind::Mismatch {
                expression: Print::to_string(exp),
                value: arena.to_string(value),
            }),
        ),
    }
}

// - List expression

fn assign_list_exp<'global>(
    arena: &mut ValueArena,
    ctx: Context<'global>,
    exps: &[ast::Exp],
    values: &[Value],
) -> Backtrack<Context<'global>> {
    assign_exps(arena, ctx, exps, values)
}

// - Cons expression

fn assign_cons_exp<'global>(
    arena: &mut ValueArena,
    ctx: Context<'global>,
    exp: &ast::Exp,
    exp_head: &ast::Exp,
    exp_tail: &ast::Exp,
    value: &Value,
    values: &[Value],
) -> Backtrack<Context<'global>> {
    let Some((value_head, values_tail)) = values.split_first() else {
        return Backtrack::err(
            exp.span.clone(),
            ErrorKind::Assign(AssignErrorKind::EmptyCons),
        );
    };
    let typ = phrase!(node: arena.typ(value).clone(), span: exp.span.clone());
    let value_tail = backtrack_from_result!(
        make::list(
            arena,
            typ.node.clone(),
            values_tail.to_vec(),
            Span::default()
        ),
        &Span::default()
    );
    let ctx = backtrack!(assign_exp(arena, ctx, exp_head, *value_head));
    assign_exp(arena, ctx, exp_tail, value_tail)
}

// - Iteration expression

fn assign_iter_exp<'global>(
    arena: &mut ValueArena,
    mut ctx: Context<'global>,
    exp: &ast::Exp,
    exp_inner: &ast::Exp,
    iter: &ast::Iter,
    vars: &[ast::Var],
    value: Value,
) -> Backtrack<Context<'global>> {
    if let Some(var) = is_iter_var_exp(exp) {
        ctx.add_value(var, value);
        return Backtrack::Ok(ctx);
    }
    let span = &exp.span;
    match iter {
        ast::Iter::Opt => {
            let value_inner = backtrack_from_result!(get::opt(arena, &value), span);
            let mut ctx = match value_inner {
                Some(value) => backtrack!(assign_exp(arena, ctx, exp_inner, value)),
                None => ctx,
            };
            for var in vars {
                let mut iters = var.iters.clone();
                iters.push(ast::Iter::Opt);
                let typ = typ::make::iterate(var.typ.clone(), &iters);
                let value_sub = if value_inner.is_some() {
                    let value = backtrack_from_result!(
                        ctx.find_value(&Variable::new(var.id.clone(), var.iters.clone())),
                        &var.id.span
                    );
                    Some(*value)
                } else {
                    None
                };
                let value_sub = backtrack_from_result!(
                    make::opt(arena, typ.node.into(), value_sub, Span::default()),
                    span
                );
                ctx.add_value(Variable::new(var.id.clone(), iters), value_sub);
            }
            Backtrack::Ok(ctx)
        }
        ast::Iter::List => {
            let values = backtrack_from_result!(get::list(arena, &value), span).to_vec();
            let ctx_sub = ctx.wipe();
            let mut ctxs = Vec::with_capacity(values.len());
            for value in values {
                ctxs.push(backtrack!(assign_exp(
                    arena,
                    ctx_sub.clone(),
                    exp_inner,
                    value
                )));
            }
            for var in vars {
                let mut iters = var.iters.clone();
                iters.push(ast::Iter::List);
                let typ = typ::make::iterate(var.typ.clone(), &iters);
                let mut values = Vec::with_capacity(ctxs.len());
                for ctx_sub in &ctxs {
                    let value = backtrack_from_result!(
                        ctx_sub.find_value(&Variable::new(var.id.clone(), var.iters.clone())),
                        &var.id.span
                    );
                    values.push(*value);
                }
                let value_sub = backtrack_from_result!(
                    make::list(arena, typ.node.into(), values, Span::default()),
                    span
                );
                ctx.add_value(Variable::new(var.id.clone(), iters), value_sub);
            }
            Backtrack::Ok(ctx)
        }
    }
}

// = Argument assignment

pub fn assign_arg<'global>(
    arena: &mut ValueArena,
    ctx_caller: &Context<'_>,
    ctx_callee: Context<'global>,
    arg: &ast::Arg,
    value: Value,
) -> Backtrack<Context<'global>> {
    match &arg.node {
        ast::ArgKind::Exp(exp) => assign_exp_arg(arena, ctx_callee, exp, value),
        ast::ArgKind::Def(id) => assign_def_arg(arena, ctx_caller, ctx_callee, id, value),
    }
}

pub fn assign_args<'global>(
    arena: &mut ValueArena,
    ctx_caller: &Context<'_>,
    ctx_callee: Context<'global>,
    args: &[ast::Arg],
    values: &[Value],
) -> Backtrack<Context<'global>> {
    if args.len() != values.len() {
        return Backtrack::err(
            Span::over(&args.iter().map(|arg| arg.span.clone()).collect::<Vec<_>>()),
            ErrorKind::Assign(AssignErrorKind::ArgumentArityMismatch {
                expected: args.len(),
                actual: values.len(),
            }),
        );
    }
    let mut ctx = ctx_callee;
    for (arg, value) in args.iter().zip(values.iter()) {
        ctx = backtrack!(assign_arg(arena, ctx_caller, ctx, arg, *value));
    }
    Backtrack::Ok(ctx)
}

// - Expression argument

fn assign_exp_arg<'global>(
    arena: &mut ValueArena,
    ctx: Context<'global>,
    exp: &ast::Exp,
    value: Value,
) -> Backtrack<Context<'global>> {
    assign_exp(arena, ctx, exp, value)
}

// - Function argument

fn assign_def_arg<'global>(
    arena: &mut ValueArena,
    ctx_caller: &Context<'_>,
    mut ctx_callee: Context<'global>,
    id: &ast::Id,
    value: Value,
) -> Backtrack<Context<'global>> {
    let ValueKind::Func(id_func) = arena.kind(&value) else {
        return Backtrack::err(
            id.span.clone(),
            ErrorKind::Assign(AssignErrorKind::DefinitionMismatch {
                value: arena.to_string(&value),
                definition: id.node.clone(),
            }),
        );
    };
    let (_, func) = backtrack_from_result!(ctx_caller.find_func(id_func), &id_func.span);
    backtrack_from_result!(ctx_callee.add_func(id.clone(), Rc::clone(func)), &id.span);
    Backtrack::Ok(ctx_callee)
}
