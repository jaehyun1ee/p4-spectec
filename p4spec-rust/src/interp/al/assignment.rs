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

use super::{
    backtrack::Backtrack,
    context::{Context, Spec},
    interpreter::{back, located},
};

pub fn is_iter_var_exp(exp: &ast::Exp) -> Option<Variable> {
    match &exp.node {
        ast::ExpKind::Var(id) => Some(Variable::new(id.clone(), vec![])),
        ast::ExpKind::Iter(exp, (iter, vars)) => {
            let mut var = is_iter_var_exp(exp)?;
            let [binding] = vars.as_slice() else {
                return None;
            };
            if var.id.node != binding.id.node || var.iters != binding.iters {
                return None;
            }
            var.iters.push(*iter);
            Some(var)
        }
        _ => None,
    }
}

pub fn assign_exp(ctx: &Context, exp: &ast::Exp, value: Rc<Value>) -> Backtrack<Context> {
    match (&exp.node, &value.node) {
        (ast::ExpKind::Var(id), _) => {
            let mut ctx = ctx.clone();
            ctx.add_value(Variable::new(id.clone(), vec![]), value);
            Backtrack::Ok(ctx)
        }
        (ast::ExpKind::Tuple(exps), ValueKind::Tuple(values))
        | (ast::ExpKind::List(exps), ValueKind::List(values)) => assign_exps(ctx, exps, values),
        (ast::ExpKind::Case(not_exp), ValueKind::Case(value_case)) => {
            let exps = not_exp.args().into_iter().cloned().collect::<Vec<_>>();
            let values = value_case.args().into_iter().cloned().collect::<Vec<_>>();
            assign_exps(ctx, &exps, &values)
        }
        (ast::ExpKind::Str(exp_fields), ValueKind::Struct(value_fields)) => {
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
        (ast::ExpKind::Opt(Some(exp)), ValueKind::Opt(Some(value))) => {
            assign_exp(ctx, exp, Rc::clone(value))
        }
        (ast::ExpKind::Opt(None), ValueKind::Opt(None)) => Backtrack::Ok(ctx.clone()),
        (ast::ExpKind::Cons(exp_h, exp_t), ValueKind::List(values)) => {
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
        (ast::ExpKind::Iter(exp_inner, (iter, vars)), _) => {
            if let Some(var) = is_iter_var_exp(exp) {
                let mut ctx = ctx.clone();
                ctx.add_value(var, value);
                return Backtrack::Ok(ctx);
            }
            match iter {
                ast::Iter::Opt => {
                    let value_inner = back!(located(get::opt(&value), &exp.span));
                    let mut ctx = match value_inner {
                        Some(value) => back!(assign_exp(ctx, exp_inner, Rc::clone(value))),
                        None => ctx.clone(),
                    };
                    for var in vars {
                        let mut iters = var.iters.clone();
                        iters.push(ast::Iter::Opt);
                        let typ = typ::make::iterate(var.typ.clone(), &iters);
                        let value_sub = if value_inner.is_some() {
                            let value = back!(located(
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
                    let values = back!(located(get::list(&value), &exp.span));
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
                            let value = back!(located(
                                ctx_sub
                                    .find_value(&Variable::new(var.id.clone(), var.iters.clone())),
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
    for (arg, value) in args.iter().zip(values) {
        match &arg.node {
            ast::ArgKind::Exp(exp) => ctx = back!(assign_exp(&ctx, exp, Rc::clone(value))),
            ast::ArgKind::Def(id) => {
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
                let (_, func) = back!(located(ctx_caller.find_func(spec, id_func), &id_func.span));
                back!(located(
                    ctx.add_func(spec, id.clone(), func.clone()),
                    &id.span
                ));
            }
        }
    }
    Backtrack::Ok(ctx)
}
