//! Expression evaluation preserves left-to-right effects and persistent path updates

use std::rc::Rc;

use num_traits::ToPrimitive;

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
    runtime::ops::{
        typ::{Theta, subst_typ},
        value,
    },
};

use super::{
    Al,
    assignment::is_iter_var_exp,
    backtrack::Backtrack,
    context::{Context, Spec},
    interpreter::{back, invoke_func, located},
};

pub(super) fn eval_exps<I: Interface, E: Extern>(
    runner: &mut RunnerContext<'_, Al, I, E>,
    ctx: &Context,
    exps: &[ast::Exp],
) -> Backtrack<Vec<Rc<Value>>> {
    let mut values = Vec::with_capacity(exps.len());
    for exp in exps {
        values.push(back!(eval_exp(runner, ctx, exp)));
    }
    Backtrack::Ok(values)
}

pub(super) fn eval_args<I: Interface, E: Extern>(
    runner: &mut RunnerContext<'_, Al, I, E>,
    ctx: &Context,
    args: &[ast::Arg],
) -> Backtrack<Vec<Rc<Value>>> {
    let mut values = Vec::with_capacity(args.len());
    for arg in args {
        let value = match &arg.node {
            ast::ArgKind::Exp(exp) => back!(eval_exp(runner, ctx, exp)),
            ast::ArgKind::Def(id) => {
                let typ = back!(located(ctx.find_func_typ(runner.spec(), id), &arg.span));
                make::func(
                    id.clone(),
                    typ.tparams,
                    typ.typs_params,
                    *typ.typ_ret,
                    Span::default(),
                )
            }
        };
        values.push(value);
    }
    Backtrack::Ok(values)
}

pub(super) fn eval_exp<I: Interface, E: Extern>(
    runner: &mut RunnerContext<'_, Al, I, E>,
    ctx: &Context,
    exp: &ast::Exp,
) -> Backtrack<Rc<Value>> {
    let span = &exp.span;
    let typ = crate::phrase!(node: exp.note.as_ref().clone(), span: span.clone());
    let value = match &exp.node {
        ast::ExpKind::Bool(value) => make::bool(*value, Span::default()),
        ast::ExpKind::Num(value) => make::num(value.clone(), Span::default()),
        ast::ExpKind::Text(value) => make::text(value.clone(), Span::default()),
        ast::ExpKind::Var(id) => {
            let var = Variable::new(id.clone(), Vec::new());
            back!(located(ctx.find_value(&var), span)).clone()
        }
        ast::ExpKind::Un(op, _, exp) => {
            let value = back!(eval_exp(runner, ctx, exp));
            match op {
                ast::UnOp::Bool(boolean::UnOp::Not) => {
                    make::bool(!back!(located(get::bool(&value), span)), Span::default())
                }
                ast::UnOp::Num(op) => {
                    let number = back!(located(get::num(&value), span));
                    make::num(num::un(*op, number), Span::default())
                }
            }
        }
        ast::ExpKind::Bin(op, _, exp_l, exp_r) => {
            let value_l = back!(eval_exp(runner, ctx, exp_l));
            let value_r = back!(eval_exp(runner, ctx, exp_r));
            match op {
                ast::BinOp::Bool(op) => {
                    let bool_l = back!(located(get::bool(&value_l), span));
                    let bool_r = back!(located(get::bool(&value_r), span));
                    let result = match op {
                        boolean::BinOp::And => bool_l && bool_r,
                        boolean::BinOp::Or => bool_l || bool_r,
                        boolean::BinOp::Impl => !bool_l || bool_r,
                        boolean::BinOp::Equiv => bool_l == bool_r,
                    };
                    make::bool(result, Span::default())
                }
                ast::BinOp::Num(op) => {
                    let num_l = back!(located(get::num(&value_l), span));
                    let num_r = back!(located(get::num(&value_r), span));
                    let number = back!(located(num::bin(*op, num_l, num_r), span));
                    make::num(number, Span::default())
                }
            }
        }
        ast::ExpKind::Cmp(op, _, exp_l, exp_r) => {
            let value_l = back!(eval_exp(runner, ctx, exp_l));
            let value_r = back!(eval_exp(runner, ctx, exp_r));
            let result = match op {
                ast::CmpOp::Bool(boolean::CmpOp::Eq) => value_l.syntax_eq(&value_r),
                ast::CmpOp::Bool(boolean::CmpOp::Ne) => !value_l.syntax_eq(&value_r),
                ast::CmpOp::Num(op) => {
                    let num_l = back!(located(get::num(&value_l), span));
                    let num_r = back!(located(get::num(&value_r), span));
                    back!(located(num::cmp(*op, num_l, num_r), span))
                }
            };
            make::bool(result, Span::default())
        }
        ast::ExpKind::UpCast(typ, exp_inner) | ast::ExpKind::DownCast(typ, exp_inner) => {
            let value = back!(eval_exp(runner, ctx, exp_inner));
            let down = matches!(&exp.node, ast::ExpKind::DownCast(_, _));
            return cast(ctx, runner.spec(), typ, value, down);
        }
        ast::ExpKind::Sub(exp, _, subcheck) => {
            let value = back!(eval_exp(runner, ctx, exp));
            let tdenv = ctx.type_env(runner.spec());
            let find_func = |name: &str| {
                let id = crate::phrase!(node: name.to_owned(), span: span.clone());
                ctx.find_func_typ(runner.spec(), &id).ok()
            };
            let matches = back!(located(
                value::check(&tdenv, &find_func, subcheck, &value),
                span
            ));
            make::bool(matches, Span::default())
        }
        ast::ExpKind::Match(exp, pattern) => {
            let value = back!(eval_exp(runner, ctx, exp));
            let matches = match (pattern, &value.node) {
                (ast::Pattern::Case(mixop), ValueKind::Case(value)) => {
                    value.eq_shape(mixop.as_ref())
                }
                (ast::Pattern::List(pattern), ValueKind::List(values)) => match pattern {
                    ListPattern::Cons => !values.is_empty(),
                    ListPattern::Fixed(len) => i64::try_from(values.len()).ok() == Some(*len),
                    ListPattern::Nil => values.is_empty(),
                },
                (ast::Pattern::Opt(OptPattern::Some), ValueKind::Opt(Some(_)))
                | (ast::Pattern::Opt(OptPattern::None), ValueKind::Opt(None)) => true,
                _ => false,
            };
            make::bool(matches, Span::default())
        }
        ast::ExpKind::Tuple(exps) => {
            make::tuple(&typ, back!(eval_exps(runner, ctx, exps)), Span::default())
        }
        ast::ExpKind::Case(not_exp) => {
            let mut values = Vec::new();
            for exp in not_exp.args() {
                values.push(back!(eval_exp(runner, ctx, exp)));
            }
            let case = back!(located(ast::Mixop::fill(&not_exp.to_mixop(), values), span));
            make::case_(&typ, case, Span::default())
        }
        ast::ExpKind::Str(fields) => {
            let mut values = Vec::with_capacity(fields.len());
            for (atom, exp) in fields {
                values.push((atom.clone(), back!(eval_exp(runner, ctx, exp))));
            }
            make::structure(&typ, values, Span::default())
        }
        ast::ExpKind::Opt(exp) => {
            let value = match exp {
                Some(exp) => Some(back!(eval_exp(runner, ctx, exp))),
                None => None,
            };
            make::opt(&typ, value, Span::default())
        }
        ast::ExpKind::List(exps) => {
            make::list(&typ, back!(eval_exps(runner, ctx, exps)), Span::default())
        }
        ast::ExpKind::Cons(exp_h, exp_t) => {
            let value_h = back!(eval_exp(runner, ctx, exp_h));
            let value_t = back!(eval_exp(runner, ctx, exp_t));
            let values_t = back!(located(get::list(&value_t), span));
            let mut values = Vec::with_capacity(values_t.len() + 1);
            values.push(value_h);
            values.extend_from_slice(values_t);
            make::list(&typ, values, Span::default())
        }
        ast::ExpKind::Cat(exp_l, exp_r) => {
            let value_l = back!(eval_exp(runner, ctx, exp_l));
            let value_r = back!(eval_exp(runner, ctx, exp_r));
            match (&value_l.node, &value_r.node) {
                (ValueKind::Text(text_l), ValueKind::Text(text_r)) => {
                    make::text(format!("{text_l}{text_r}"), Span::default())
                }
                (ValueKind::List(values_l), ValueKind::List(values_r)) => {
                    let mut values = values_l.clone();
                    values.extend_from_slice(values_r);
                    make::list(&typ, values, Span::default())
                }
                _ => {
                    return Backtrack::err(
                        Span::over(&[exp_l.span.clone(), exp_r.span.clone()]),
                        "concatenation expects either two texts or two lists",
                    );
                }
            }
        }
        ast::ExpKind::Mem(exp_e, exp_s) => {
            let value_e = back!(eval_exp(runner, ctx, exp_e));
            let value_s = back!(eval_exp(runner, ctx, exp_s));
            let values = back!(located(get::list(&value_s), span));
            make::bool(
                values.iter().any(|value| value.syntax_eq(&value_e)),
                Span::default(),
            )
        }
        ast::ExpKind::Len(exp) => {
            let value = back!(eval_exp(runner, ctx, exp));
            let len = match &value.node {
                ValueKind::Text(text) => text.len(),
                ValueKind::List(values) => values.len(),
                _ => {
                    return Backtrack::err(
                        exp.span.clone(),
                        "length operation expects either a text or a list",
                    );
                }
            };
            make::nat((len as u64).into(), Span::default())
        }
        ast::ExpKind::Dot(exp_b, atom) => {
            let value = back!(eval_exp(runner, ctx, exp_b));
            back!(dot(&value, atom, span))
        }
        ast::ExpKind::Idx(exp_b, exp_i) => {
            let value = back!(eval_exp(runner, ctx, exp_b));
            let idx = back!(eval_index(runner, ctx, exp_i));
            back!(index(&value, idx, &exp_b.span, &exp_i.span))
        }
        ast::ExpKind::Slice(exp_b, exp_i, exp_n) => {
            let value = back!(eval_exp(runner, ctx, exp_b));
            let idx = back!(eval_index(runner, ctx, exp_i));
            let len = back!(eval_index(runner, ctx, exp_n));
            let bounds_span = if matches!(value.node, ValueKind::Text(_)) {
                &exp_i.span
            } else {
                &exp_n.span
            };
            back!(slice(&value, idx, len, &typ, &exp_b.span, bounds_span))
        }
        ast::ExpKind::Upd(exp_b, path, exp_f) => {
            let value_b = back!(eval_exp(runner, ctx, exp_b));
            let value_f = back!(eval_exp(runner, ctx, exp_f));
            return eval_update_path(runner, ctx, &value_b, path, value_f);
        }
        ast::ExpKind::Call(id, targs, args) => {
            let theta = ctx.local_theta();
            let mut typs = Vec::with_capacity(targs.len());
            for targ in targs {
                typs.push(back!(located(subst_typ(&theta, targ), &targ.span)));
            }
            let values = back!(eval_args(runner, ctx, args));
            return invoke_func(runner, ctx, id, &typs, &values);
        }
        ast::ExpKind::Iter(exp_inner, (iter, vars)) => {
            if let Some(var) = is_iter_var_exp(exp) {
                return Backtrack::Ok(back!(located(ctx.find_value(&var), span)).clone());
            }
            match iter {
                ast::Iter::Opt => {
                    let ctx_sub = back!(located(ctx.sub_opt(vars), span));
                    let value = match ctx_sub {
                        Some(ctx_sub) => Some(back!(eval_exp(runner, &ctx_sub, exp_inner))),
                        None => None,
                    };
                    make::opt(&typ, value, Span::default())
                }
                ast::Iter::List => {
                    let ctxs_sub = back!(located(ctx.sub_list(vars), span));
                    let mut values = Vec::with_capacity(ctxs_sub.len());
                    for ctx_sub in ctxs_sub {
                        values.push(back!(eval_exp(runner, &ctx_sub, exp_inner)));
                    }
                    make::list(&typ, values, Span::default())
                }
            }
        }
    };
    Backtrack::Ok(value)
}

fn cast(
    ctx: &Context,
    spec: &Spec,
    typ: &ast::Typ,
    value: Rc<Value>,
    down: bool,
) -> Backtrack<Rc<Value>> {
    let span = &typ.span;
    let result = match &typ.node {
        ast::TypKind::Num(num::Typ::Int) if !down => {
            let number = back!(located(get::num(&value), span));
            match number {
                num::Number::Nat(number) => make::int(number.as_bigint().clone(), Span::default()),
                num::Number::Int(_) => value,
            }
        }
        ast::TypKind::Num(num::Typ::Nat) if down => {
            let number = back!(located(get::num(&value), span));
            match number {
                num::Number::Nat(_) => value,
                num::Number::Int(number) => {
                    let number = back!(located(num::Natural::try_from(number.clone()), span));
                    make::nat(number, Span::default())
                }
            }
        }
        ast::TypKind::Var(id, targs) => {
            let (tparams, def_typ) = back!(located(ctx.find_defined_typdef(spec, id), span));
            let theta = back!(located(Theta::from_lists(tparams, targs), span));
            match &def_typ.node {
                ast::DefTypKind::Plain(typ) => {
                    let typ = back!(located(subst_typ(&theta, typ), span));
                    return cast(ctx, spec, &typ, value, down);
                }
                _ => value,
            }
        }
        ast::TypKind::Tuple(typs) => {
            let values = back!(located(get::tuple(&value), span));
            if typs.len() != values.len() {
                return Backtrack::err(span.clone(), "tuple cast arity mismatch");
            }
            let mut values_cast = Vec::with_capacity(values.len());
            for (typ, value) in typs.iter().zip(values) {
                values_cast.push(back!(cast(ctx, spec, typ, value.clone(), down)));
            }
            make::tuple(typ, values_cast, Span::default())
        }
        ast::TypKind::Iter(typ_inner, ast::Iter::Opt) => {
            let value = back!(located(get::opt(&value), span));
            let value = match value {
                Some(value) => Some(back!(cast(ctx, spec, typ_inner, value.clone(), down))),
                None => None,
            };
            make::opt(typ_inner, value, Span::default())
        }
        ast::TypKind::Iter(typ_inner, ast::Iter::List) => {
            let values = back!(located(get::list(&value), span));
            let mut values_cast = Vec::with_capacity(values.len());
            for value in values {
                values_cast.push(back!(cast(ctx, spec, typ_inner, value.clone(), down)));
            }
            make::list(typ_inner, values_cast, Span::default())
        }
        _ => value,
    };
    Backtrack::Ok(result)
}

fn eval_index<I: Interface, E: Extern>(
    runner: &mut RunnerContext<'_, Al, I, E>,
    ctx: &Context,
    exp: &ast::Exp,
) -> Backtrack<i64> {
    let value = back!(eval_exp(runner, ctx, exp));
    let number = back!(located(get::num(&value), &exp.span));
    let idx = num::to_int(number).to_i64();
    located(idx.ok_or("index does not fit a machine integer"), &exp.span)
}

fn dot(value: &Value, atom: &ast::Atom, span: &Span) -> Backtrack<Rc<Value>> {
    let fields = back!(located(get::structure(value), span));
    match fields.iter().find(|(field, _)| field.node == atom.node) {
        Some((_, value)) => Backtrack::Ok(value.clone()),
        None => Backtrack::err(atom.span.clone(), "undefined structure field"),
    }
}

fn text_slice(text: &str, start: usize, end: usize, span: &Span) -> Backtrack<String> {
    match text.get(start..end) {
        Some(text) => Backtrack::Ok(text.to_owned()),
        None => Backtrack::err(span.clone(), "text byte slice is not on UTF-8 boundaries"),
    }
}

fn index(value: &Value, idx: i64, base_span: &Span, index_span: &Span) -> Backtrack<Rc<Value>> {
    let len = match &value.node {
        ValueKind::Text(text) => text.len(),
        ValueKind::List(values) => values.len(),
        _ => {
            return Backtrack::err(
                base_span.clone(),
                "indexing expects either a text or a list",
            );
        }
    };
    if idx < 0 || idx as u64 >= len as u64 {
        return Backtrack::err(
            index_span.clone(),
            format!("index {idx} out of bounds [0, {len})"),
        );
    }
    match &value.node {
        ValueKind::Text(text) => {
            let text = back!(text_slice(text, idx as usize, idx as usize + 1, index_span));
            Backtrack::Ok(make::text(text, Span::default()))
        }
        ValueKind::List(values) => Backtrack::Ok(values[idx as usize].clone()),
        _ => unreachable!(),
    }
}

fn slice(
    value: &Value,
    idx: i64,
    len: i64,
    typ: &ast::Typ,
    base_span: &Span,
    bounds_span: &Span,
) -> Backtrack<Rc<Value>> {
    let end = back!(located(
        idx.checked_add(len)
            .ok_or("slice end overflows a machine integer"),
        bounds_span
    ));
    let size = match &value.node {
        ValueKind::Text(text) => text.len(),
        ValueKind::List(values) => values.len(),
        _ => return Backtrack::err(base_span.clone(), "slicing expects either a text or a list"),
    };
    if idx < 0 || end > size as i64 {
        return Backtrack::err(
            bounds_span.clone(),
            format!("slice [{idx}, {end}) out of bounds [0, {size})"),
        );
    }
    match &value.node {
        ValueKind::Text(text) => {
            if len < 0 {
                return Backtrack::err(bounds_span.clone(), "text slice length is negative");
            }
            let text = back!(text_slice(text, idx as usize, end as usize, bounds_span));
            Backtrack::Ok(make::text(text, Span::default()))
        }
        ValueKind::List(values) => {
            let values = values
                .iter()
                .enumerate()
                .filter(|(i, _)| idx <= *i as i64 && (*i as i64) < end)
                .map(|(_, value)| value.clone())
                .collect();
            Backtrack::Ok(make::list(typ, values, Span::default()))
        }
        _ => unreachable!(),
    }
}

fn eval_access_path<I: Interface, E: Extern>(
    runner: &mut RunnerContext<'_, Al, I, E>,
    ctx: &Context,
    value_b: &Rc<Value>,
    path: &ast::Path,
) -> Backtrack<Rc<Value>> {
    match &path.node {
        ast::PathKind::Root => Backtrack::Ok(value_b.clone()),
        ast::PathKind::Idx(path, exp_i) => {
            let value = back!(eval_access_path(runner, ctx, value_b, path));
            let idx = back!(eval_index(runner, ctx, exp_i));
            index(&value, idx, &path.span, &exp_i.span)
        }
        ast::PathKind::Slice(path, exp_i, exp_n) => {
            let typ = crate::phrase!(node: path.note.as_ref().clone(), span: path.span.clone());
            let value = back!(eval_access_path(runner, ctx, value_b, path));
            let idx = back!(eval_index(runner, ctx, exp_i));
            let len = back!(eval_index(runner, ctx, exp_n));
            slice(&value, idx, len, &typ, &path.span, &exp_n.span)
        }
        ast::PathKind::Dot(path, atom) => {
            let value = back!(eval_access_path(runner, ctx, value_b, path));
            dot(&value, atom, &path.span)
        }
    }
}

fn eval_update_path<I: Interface, E: Extern>(
    runner: &mut RunnerContext<'_, Al, I, E>,
    ctx: &Context,
    value_b: &Rc<Value>,
    path: &ast::Path,
    value_upd: Rc<Value>,
) -> Backtrack<Rc<Value>> {
    match &path.node {
        ast::PathKind::Root => Backtrack::Ok(value_upd),
        ast::PathKind::Idx(path, exp_i) => {
            let typ = crate::phrase!(node: path.note.as_ref().clone(), span: path.span.clone());
            let value = back!(eval_access_path(runner, ctx, value_b, path));
            let idx = back!(eval_index(runner, ctx, exp_i));
            back!(index(&value, idx, &path.span, &exp_i.span));
            let value = match &value.node {
                ValueKind::Text(text) => {
                    let text_upd = back!(located(get::text(&value_upd), &exp_i.span));
                    if text_upd.len() != 1 {
                        return Backtrack::err(
                            exp_i.span.clone(),
                            "updating a character requires a single-character text",
                        );
                    }
                    let left = back!(text_slice(text, 0, idx as usize, &exp_i.span));
                    let right = back!(text_slice(text, idx as usize + 1, text.len(), &exp_i.span));
                    make::text(format!("{left}{text_upd}{right}"), Span::default())
                }
                ValueKind::List(values) => {
                    let mut values = values.clone();
                    values[idx as usize] = value_upd;
                    make::list(&typ, values, Span::default())
                }
                _ => unreachable!(),
            };
            eval_update_path(runner, ctx, value_b, path, value)
        }
        ast::PathKind::Slice(path, exp_i, exp_n) => {
            let typ = crate::phrase!(node: path.note.as_ref().clone(), span: path.span.clone());
            let value = back!(eval_access_path(runner, ctx, value_b, path));
            let idx = back!(eval_index(runner, ctx, exp_i));
            let len = back!(eval_index(runner, ctx, exp_n));
            let end = back!(located(
                idx.checked_add(len)
                    .ok_or("slice end overflows a machine integer"),
                &exp_n.span
            ));
            let size = match &value.node {
                ValueKind::Text(text) => text.len(),
                ValueKind::List(values) => values.len(),
                _ => {
                    return Backtrack::err(
                        path.span.clone(),
                        "slicing expects either a text or a list",
                    );
                }
            };
            if idx < 0 || end > size as i64 {
                return Backtrack::err(
                    exp_n.span.clone(),
                    format!("slice [{idx}, {end}) out of bounds [0, {size})"),
                );
            }
            let value = match &value.node {
                ValueKind::Text(text) => {
                    let text_upd = back!(located(get::text(&value_upd), &exp_n.span));
                    if len < 0 || text_upd.len() as i64 != len {
                        return Backtrack::err(
                            exp_n.span.clone(),
                            format!(
                                "updating a slice of length {len} requires a text of length {len}, but got length {}",
                                text_upd.len()
                            ),
                        );
                    }
                    let left = back!(text_slice(text, 0, idx as usize, &exp_n.span));
                    let right = back!(text_slice(text, end as usize, text.len(), &exp_n.span));
                    make::text(format!("{left}{text_upd}{right}"), Span::default())
                }
                ValueKind::List(values) => {
                    let values_upd = back!(located(get::list(&value_upd), &exp_n.span));
                    if len < 0 || values_upd.len() as i64 != len {
                        return Backtrack::err(
                            exp_n.span.clone(),
                            format!(
                                "updating a slice of length {len} requires a list of length {len}, but got length {}",
                                values_upd.len()
                            ),
                        );
                    }
                    let mut values = values.clone();
                    for (i, value) in values.iter_mut().enumerate() {
                        if idx <= i as i64 && (i as i64) < end {
                            *value = values_upd[i - idx as usize].clone();
                        }
                    }
                    make::list(&typ, values, Span::default())
                }
                _ => unreachable!(),
            };
            eval_update_path(runner, ctx, value_b, path, value)
        }
        ast::PathKind::Dot(path, atom) => {
            let typ = crate::phrase!(node: path.note.as_ref().clone(), span: path.span.clone());
            let value = back!(eval_access_path(runner, ctx, value_b, path));
            let fields = back!(located(get::structure(&value), &path.span));
            let fields = fields
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
            let value = make::structure(&typ, fields, Span::default());
            eval_update_path(runner, ctx, value_b, path, value)
        }
    }
}
