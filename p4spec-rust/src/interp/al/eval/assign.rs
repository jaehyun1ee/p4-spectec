//! Destructuring assignments preserve iteration paths and isolate list rows

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
    context::{Context, Spec},
    util::is_iter_var_exp,
};

// = Expression assignment

pub fn assign_exp(ctx: &Context, exp: &ast::Exp, value: Rc<Value>) -> Backtrack<Context> {
    match (&exp.node, &value.node) {
        (ast::ExpKind::Var(id), _) => assign_var_exp(ctx, id, value),
        (ast::ExpKind::Tuple(exps), ValueKind::Tuple(values)) => {
            assign_tuple_exp(ctx, exps, values)
        }
        (ast::ExpKind::List(exps), ValueKind::List(values)) => assign_list_exp(ctx, exps, values),
        (ast::ExpKind::Case(not_exp), ValueKind::Case(value_case)) => {
            assign_case_exp(ctx, not_exp, value_case)
        }
        (ast::ExpKind::Str(exp_fields), ValueKind::Struct(value_fields)) => {
            assign_str_exp(ctx, exp_fields, value_fields)
        }
        (ast::ExpKind::Opt(exp_opt), ValueKind::Opt(value_opt)) => {
            assign_opt_exp(ctx, exp, exp_opt, &value, value_opt)
        }
        (ast::ExpKind::Cons(exp_h, exp_t), ValueKind::List(values)) => {
            assign_cons_exp(ctx, exp, exp_h, exp_t, &value, values)
        }
        (ast::ExpKind::Iter(exp_inner, (iter, vars)), _) => {
            if let Some(var) = is_iter_var_exp(exp) {
                let mut ctx = ctx.clone();
                ctx.add_value(var, value);
                return Backtrack::Ok(ctx);
            }
            assign_iter_exp(ctx, exp_inner, iter, vars, value, &exp.span)
        }
        _ => Backtrack::err(
            exp.span.clone(),
            format!(
                "match failed {} <- {}",
                Print::to_string(exp),
                Print::to_string(value.as_ref())
            ),
        ),
    }
}

pub fn assign_exps(ctx: &Context, exps: &[ast::Exp], values: &[Rc<Value>]) -> Backtrack<Context> {
    if exps.len() != values.len() {
        return Backtrack::err(
            Span::over(&exps.iter().map(|exp| exp.span.clone()).collect::<Vec<_>>()),
            format!(
                "mismatch in number of expressions and values while assigning, expected {} value(s) but got {}",
                exps.len(),
                values.len()
            ),
        );
    }
    let mut ctx = ctx.clone();
    for (exp, value) in exps.iter().zip(values) {
        ctx = back!(assign_exp(&ctx, exp, Rc::clone(value)));
    }
    Backtrack::Ok(ctx)
}

// - Variable expression

fn assign_var_exp(ctx: &Context, id: &ast::Id, value: Rc<Value>) -> Backtrack<Context> {
    let mut ctx = ctx.clone();
    ctx.add_value(Variable::new(id.clone(), vec![]), value);
    Backtrack::Ok(ctx)
}

// - Tuple expression

fn assign_tuple_exp(ctx: &Context, exps: &[ast::Exp], values: &[Rc<Value>]) -> Backtrack<Context> {
    assign_exps(ctx, exps, values)
}

// - List expression

fn assign_list_exp(ctx: &Context, exps: &[ast::Exp], values: &[Rc<Value>]) -> Backtrack<Context> {
    assign_exps(ctx, exps, values)
}

// - Case expression

fn assign_case_exp(
    ctx: &Context,
    not_exp: &ast::NotExp,
    value_case: &ast::ValueCase,
) -> Backtrack<Context> {
    let exps = not_exp.args().into_iter().cloned().collect::<Vec<_>>();
    let values = value_case.args().into_iter().cloned().collect::<Vec<_>>();
    assign_exps(ctx, &exps, &values)
}

// - Struct expression

fn assign_str_exp(
    ctx: &Context,
    exp_fields: &[ast::ExpField],
    value_fields: &[ast::ValueField],
) -> Backtrack<Context> {
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

fn assign_opt_exp(
    ctx: &Context,
    exp: &ast::Exp,
    exp_opt: &Option<Box<ast::Exp>>,
    value: &Value,
    value_opt: &Option<Rc<Value>>,
) -> Backtrack<Context> {
    match (exp_opt, value_opt) {
        (Some(exp), Some(value)) => assign_exp(ctx, exp, Rc::clone(value)),
        (None, None) => Backtrack::Ok(ctx.clone()),
        _ => Backtrack::err(
            exp.span.clone(),
            format!(
                "match failed {} <- {}",
                Print::to_string(exp),
                Print::to_string(value)
            ),
        ),
    }
}

// - Cons expression

fn assign_cons_exp(
    ctx: &Context,
    exp: &ast::Exp,
    exp_h: &ast::Exp,
    exp_t: &ast::Exp,
    value: &Value,
    values: &[Rc<Value>],
) -> Backtrack<Context> {
    let Some((value_h, values_t)) = values.split_first() else {
        return Backtrack::err(
            exp.span.clone(),
            "cannot assign an empty list to a cons expression",
        );
    };
    let typ = phrase!(node: value.note.clone(), span: exp.span.clone());
    let value_t = make::list(&typ, values_t.to_vec(), Span::default());
    let ctx = back!(assign_exp(ctx, exp_h, Rc::clone(value_h)));
    assign_exp(&ctx, exp_t, value_t)
}

// - Iter expression

fn assign_iter_exp(
    ctx: &Context,
    exp_inner: &ast::Exp,
    iter: &ast::Iter,
    vars: &[ast::Var],
    value: Rc<Value>,
    span: &Span,
) -> Backtrack<Context> {
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

pub fn assign_arg(
    spec: &Spec,
    ctx_caller: &Context,
    ctx_callee: &Context,
    arg: &ast::Arg,
    value: Rc<Value>,
) -> Backtrack<Context> {
    match &arg.node {
        ast::ArgKind::Exp(exp) => assign_exp_arg(ctx_callee, exp, value),
        ast::ArgKind::Def(id) => assign_def_arg(spec, ctx_caller, ctx_callee, id, value),
    }
}

pub fn assign_args(
    spec: &Spec,
    ctx_caller: &Context,
    ctx_callee: &Context,
    args: &[ast::Arg],
    values: &[Rc<Value>],
) -> Backtrack<Context> {
    if args.len() != values.len() {
        return Backtrack::err(
            Span::over(&args.iter().map(|arg| arg.span.clone()).collect::<Vec<_>>()),
            format!(
                "mismatch in number of arguments and values while assigning, expected {} value(s) but got {}",
                args.len(),
                values.len()
            ),
        );
    }
    let mut ctx = ctx_callee.clone();
    for (arg, value) in args.iter().zip(values.iter()) {
        ctx = back!(assign_arg(spec, ctx_caller, &ctx, arg, Rc::clone(value)));
    }
    Backtrack::Ok(ctx)
}

// - Expression argument

fn assign_exp_arg(ctx: &Context, exp: &ast::Exp, value: Rc<Value>) -> Backtrack<Context> {
    assign_exp(ctx, exp, value)
}

// - Function argument

fn assign_def_arg(
    spec: &Spec,
    ctx_caller: &Context,
    ctx_callee: &Context,
    id: &ast::Id,
    value: Rc<Value>,
) -> Backtrack<Context> {
    let ValueKind::Func(id_func) = &value.node else {
        return Backtrack::err(
            id.span.clone(),
            format!(
                "cannot assign a value {} to a definition {}",
                Print::to_string(value.as_ref()),
                id.node
            ),
        );
    };
    let (_, func) = back!(Backtrack::from_result(
        ctx_caller.find_func(spec, id_func),
        &id_func.span
    ));
    let mut ctx_callee = ctx_callee.clone();
    back!(Backtrack::from_result(
        ctx_callee.add_func(spec, id.clone(), func.clone()),
        &id.span
    ));
    Backtrack::Ok(ctx_callee)
}
