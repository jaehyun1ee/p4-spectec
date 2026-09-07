//! Destructuring assignments preserve iteration paths and isolate list rows

use crate::interp::al::error::AssignErrorKind;
use std::rc::Rc;

use crate::{
    lang::{
        al::ast,
        common::{Variable, source::Span},
        data::{
            typ,
            value::{Value, ValueKind, get, make},
        },
        traits::print::Print,
    },
    phrase,
};

use super::super::{
    backtrack::{Backtrack, back},
    context::Context,
    error::ErrorKind,
    util::is_iter_var_exp,
};

// = Expression assignment

pub fn assign_exp<'global>(
    ctx: &Context<'global>,
    exp: &ast::Exp,
    value: Rc<Value>,
) -> Backtrack<Context<'global>> {
    match (&exp.node, &value.node) {
        (ast::ExpKind::Var(id), _) => assign_var_exp(ctx, id, value),
        (ast::ExpKind::Tuple(exps), ValueKind::Tuple(values)) => {
            assign_tuple_exp(ctx, exps, values)
        }
        (ast::ExpKind::Case(not_exp), ValueKind::Case(value_case)) => {
            assign_case_exp(ctx, not_exp, value_case)
        }
        (ast::ExpKind::Str(exp_fields), ValueKind::Struct(value_fields)) => {
            assign_str_exp(ctx, exp_fields, value_fields)
        }
        (ast::ExpKind::Opt(exp_opt), ValueKind::Opt(value_opt)) => {
            assign_opt_exp(ctx, exp, exp_opt, &value, value_opt)
        }
        (ast::ExpKind::List(exps), ValueKind::List(values)) => assign_list_exp(ctx, exps, values),
        (ast::ExpKind::Cons(exp_head, exp_tail), ValueKind::List(values)) => {
            assign_cons_exp(ctx, exp, exp_head, exp_tail, &value, values)
        }
        (ast::ExpKind::Iter(exp_inner, (iter, vars)), _) => {
            assign_iter_exp(ctx, exp, exp_inner, iter, vars, value)
        }
        _ => Backtrack::err(
            exp.span.clone(),
            ErrorKind::Assign(AssignErrorKind::Mismatch {
                expression: Print::to_string(exp),
                value: Print::to_string(value.as_ref()),
            }),
        ),
    }
}

pub fn assign_exps<'global>(
    ctx: &Context<'global>,
    exps: &[ast::Exp],
    values: &[Rc<Value>],
) -> Backtrack<Context<'global>> {
    if exps.len() != values.len() {
        return Backtrack::err(
            Span::over(&exps.iter().map(|exp| exp.span.clone()).collect::<Vec<_>>()),
            ErrorKind::Assign(AssignErrorKind::ExpressionArityMismatch {
                expected: exps.len(),
                actual: values.len(),
            }),
        );
    }
    let mut ctx = ctx.clone();
    for (exp, value) in exps.iter().zip(values) {
        ctx = back!(assign_exp(&ctx, exp, Rc::clone(value)));
    }
    Backtrack::Ok(ctx)
}

// - Variable expression

fn assign_var_exp<'global>(
    ctx: &Context<'global>,
    id: &ast::Id,
    value: Rc<Value>,
) -> Backtrack<Context<'global>> {
    let mut ctx = ctx.clone();
    ctx.add_value(Variable::new(id.clone(), vec![]), value);
    Backtrack::Ok(ctx)
}

// - Tuple expression

fn assign_tuple_exp<'global>(
    ctx: &Context<'global>,
    exps: &[ast::Exp],
    values: &[Rc<Value>],
) -> Backtrack<Context<'global>> {
    assign_exps(ctx, exps, values)
}

// - Case expression

fn assign_case_exp<'global>(
    ctx: &Context<'global>,
    not_exp: &ast::NotExp,
    value_case: &ast::ValueCase,
) -> Backtrack<Context<'global>> {
    let exps = not_exp.args().into_iter().cloned().collect::<Vec<_>>();
    let values = value_case.args().into_iter().cloned().collect::<Vec<_>>();
    assign_exps(ctx, &exps, &values)
}

// - Struct expression

fn assign_str_exp<'global>(
    ctx: &Context<'global>,
    exp_fields: &[ast::ExpField],
    value_fields: &[ast::ValueField],
) -> Backtrack<Context<'global>> {
    let exps = exp_fields
        .iter()
        .map(|(_, exp)| exp.clone())
        .collect::<Vec<_>>();
    let values = value_fields
        .iter()
        .map(|(_, value)| Rc::clone(value))
        .collect::<Vec<_>>();
    assign_exps(ctx, &exps, &values)
}

// - Optional expression

fn assign_opt_exp<'global>(
    ctx: &Context<'global>,
    exp: &ast::Exp,
    exp_opt: &Option<Box<ast::Exp>>,
    value: &Value,
    value_opt: &Option<Rc<Value>>,
) -> Backtrack<Context<'global>> {
    match (exp_opt, value_opt) {
        (Some(exp), Some(value)) => assign_exp(ctx, exp, Rc::clone(value)),
        (None, None) => Backtrack::Ok(ctx.clone()),
        _ => Backtrack::err(
            exp.span.clone(),
            ErrorKind::Assign(AssignErrorKind::Mismatch {
                expression: Print::to_string(exp),
                value: Print::to_string(value),
            }),
        ),
    }
}

// - List expression

fn assign_list_exp<'global>(
    ctx: &Context<'global>,
    exps: &[ast::Exp],
    values: &[Rc<Value>],
) -> Backtrack<Context<'global>> {
    assign_exps(ctx, exps, values)
}

// - Cons expression

fn assign_cons_exp<'global>(
    ctx: &Context<'global>,
    exp: &ast::Exp,
    exp_head: &ast::Exp,
    exp_tail: &ast::Exp,
    value: &Value,
    values: &[Rc<Value>],
) -> Backtrack<Context<'global>> {
    let Some((value_head, values_tail)) = values.split_first() else {
        return Backtrack::err(
            exp.span.clone(),
            ErrorKind::Assign(AssignErrorKind::EmptyCons),
        );
    };
    let typ = phrase!(node: value.note.clone(), span: exp.span.clone());
    let value_tail = make::list(&typ, values_tail.to_vec(), Span::default());
    let ctx = back!(assign_exp(ctx, exp_head, Rc::clone(value_head)));
    assign_exp(&ctx, exp_tail, value_tail)
}

// - Iteration expression

fn assign_iter_exp<'global>(
    ctx: &Context<'global>,
    exp: &ast::Exp,
    exp_inner: &ast::Exp,
    iter: &ast::Iter,
    vars: &[ast::Var],
    value: Rc<Value>,
) -> Backtrack<Context<'global>> {
    if let Some(var) = is_iter_var_exp(exp) {
        let mut ctx = ctx.clone();
        ctx.add_value(var, value);
        return Backtrack::Ok(ctx);
    }
    let span = &exp.span;
    match iter {
        ast::Iter::Opt => {
            let value_inner = back!(Backtrack::from_result(get::opt(&value), span));
            let mut ctx = match value_inner {
                Some(value) => back!(assign_exp(ctx, exp_inner, Rc::clone(value))),
                None => ctx.clone(),
            };
            for var in vars {
                let mut iters = var.iters.clone();
                iters.push(ast::Iter::Opt);
                let typ = typ::make::iterate(var.typ.clone(), &iters);
                let value_sub = if value_inner.is_some() {
                    let value = back!(Backtrack::from_result(
                        ctx.find_value(&Variable::new(var.id.clone(), var.iters.clone())),
                        &var.id.span
                    ));
                    Some(Rc::clone(value))
                } else {
                    None
                };
                let value_sub = make::opt(&typ, value_sub, Span::default());
                ctx.add_value(Variable::new(var.id.clone(), iters), value_sub);
            }
            Backtrack::Ok(ctx)
        }
        ast::Iter::List => {
            let values = back!(Backtrack::from_result(get::list(&value), span));
            let ctx_sub = ctx.without_values();
            let mut ctxs = Vec::with_capacity(values.len());
            for value in values {
                ctxs.push(back!(assign_exp(&ctx_sub, exp_inner, Rc::clone(value))));
            }
            let mut ctx = ctx.clone();
            for var in vars {
                let mut iters = var.iters.clone();
                iters.push(ast::Iter::List);
                let typ = typ::make::iterate(var.typ.clone(), &iters);
                let mut values = Vec::with_capacity(ctxs.len());
                for ctx_sub in &ctxs {
                    let value = back!(Backtrack::from_result(
                        ctx_sub.find_value(&Variable::new(var.id.clone(), var.iters.clone())),
                        &var.id.span
                    ));
                    values.push(Rc::clone(value));
                }
                let value_sub = make::list(&typ, values, Span::default());
                ctx.add_value(Variable::new(var.id.clone(), iters), value_sub);
            }
            Backtrack::Ok(ctx)
        }
    }
}

// = Argument assignment

pub fn assign_arg<'global>(
    ctx_caller: &Context<'_>,
    ctx_callee: &Context<'global>,
    arg: &ast::Arg,
    value: Rc<Value>,
) -> Backtrack<Context<'global>> {
    match &arg.node {
        ast::ArgKind::Exp(exp) => assign_exp_arg(ctx_callee, exp, value),
        ast::ArgKind::Def(id) => assign_def_arg(ctx_caller, ctx_callee, id, value),
    }
}

pub fn assign_args<'global>(
    ctx_caller: &Context<'_>,
    ctx_callee: &Context<'global>,
    args: &[ast::Arg],
    values: &[Rc<Value>],
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
    let mut ctx = ctx_callee.clone();
    for (arg, value) in args.iter().zip(values.iter()) {
        ctx = back!(assign_arg(ctx_caller, &ctx, arg, Rc::clone(value)));
    }
    Backtrack::Ok(ctx)
}

// - Expression argument

fn assign_exp_arg<'global>(
    ctx: &Context<'global>,
    exp: &ast::Exp,
    value: Rc<Value>,
) -> Backtrack<Context<'global>> {
    assign_exp(ctx, exp, value)
}

// - Function argument

fn assign_def_arg<'global>(
    ctx_caller: &Context<'_>,
    ctx_callee: &Context<'global>,
    id: &ast::Id,
    value: Rc<Value>,
) -> Backtrack<Context<'global>> {
    let ValueKind::Func(id_func) = &value.node else {
        return Backtrack::err(
            id.span.clone(),
            ErrorKind::Assign(AssignErrorKind::DefinitionMismatch {
                value: Print::to_string(value.as_ref()),
                definition: id.node.clone(),
            }),
        );
    };
    let (_, func) = back!(Backtrack::from_result(
        ctx_caller.find_func(id_func),
        &id_func.span
    ));
    let mut ctx_callee = ctx_callee.clone();
    back!(Backtrack::from_result(
        ctx_callee.add_func(id.clone(), func.clone()),
        &id.span
    ));
    Backtrack::Ok(ctx_callee)
}
