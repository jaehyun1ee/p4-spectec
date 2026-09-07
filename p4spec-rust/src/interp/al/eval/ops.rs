//! Value operations shared by AL expression and path evaluation

use std::rc::Rc;

use num_traits::ToPrimitive;

use crate::{
    lang::{
        al::ast,
        common::source::Span,
        data::value::{Value, ValueKind, get, make},
        xl::num,
    },
    runtime::ops::typ::{Theta, subst_typ},
};

use super::super::{
    backtrack::{Backtrack, back},
    context::Context,
    error::{ErrorKind, ExprErrorKind},
};

// = Value operations

// - Upcast

pub(super) fn cast_up(ctx: &Context<'_>, typ: &ast::Typ, value: Rc<Value>) -> Backtrack<Rc<Value>> {
    let span = &typ.span;
    let result = match &typ.node {
        ast::TypKind::Num(num::Typ::Int) => {
            let num = back!(Backtrack::from_result(get::num(&value), span));
            match num {
                num::Number::Nat(num) => make::int(num.as_bigint().clone(), Span::default()),
                num::Number::Int(_) => value,
            }
        }
        ast::TypKind::Var(id, targs) => {
            let (tparams, def_typ) =
                back!(Backtrack::from_result(ctx.find_defined_typdef(id), span));
            let theta = back!(Backtrack::from_result(
                Theta::from_lists(tparams, targs),
                span
            ));
            match &def_typ.node {
                ast::DefTypKind::Plain(typ) => {
                    let typ = back!(Backtrack::from_result(subst_typ(&theta, typ), span));
                    return cast_up(ctx, &typ, value);
                }
                _ => value,
            }
        }
        ast::TypKind::Tuple(typs) => {
            let values = back!(Backtrack::from_result(get::tuple(&value), span));
            if typs.len() != values.len() {
                return Backtrack::err(
                    span.clone(),
                    ErrorKind::Expr(ExprErrorKind::TupleCastArityMismatch {
                        expected: typs.len(),
                        actual: values.len(),
                    }),
                );
            }
            let mut values_cast = Vec::with_capacity(values.len());
            for (typ, value) in typs.iter().zip(values) {
                values_cast.push(back!(cast_up(ctx, typ, value.clone())));
            }
            make::tuple(typ, values_cast, Span::default())
        }
        ast::TypKind::Iter(typ_inner, ast::Iter::Opt) => {
            let value = back!(Backtrack::from_result(get::opt(&value), span));
            let value = match value {
                Some(value) => Some(back!(cast_up(ctx, typ_inner, value.clone()))),
                None => None,
            };
            make::opt(typ_inner, value, Span::default())
        }
        ast::TypKind::Iter(typ_inner, ast::Iter::List) => {
            let values = back!(Backtrack::from_result(get::list(&value), span));
            let mut values_cast = Vec::with_capacity(values.len());
            for value in values {
                values_cast.push(back!(cast_up(ctx, typ_inner, value.clone())));
            }
            make::list(typ_inner, values_cast, Span::default())
        }
        _ => value,
    };
    Backtrack::Ok(result)
}

// - Downcast

pub(super) fn cast_down(
    ctx: &Context<'_>,
    typ: &ast::Typ,
    value: Rc<Value>,
) -> Backtrack<Rc<Value>> {
    let span = &typ.span;
    let result = match &typ.node {
        ast::TypKind::Num(num::Typ::Nat) => {
            let num = back!(Backtrack::from_result(get::num(&value), span));
            match num {
                num::Number::Nat(_) => value,
                num::Number::Int(num) => {
                    let num = back!(Backtrack::from_result(
                        num::Natural::try_from(num.clone()),
                        span
                    ));
                    make::nat(num, Span::default())
                }
            }
        }
        ast::TypKind::Var(id, targs) => {
            let (tparams, def_typ) =
                back!(Backtrack::from_result(ctx.find_defined_typdef(id), span));
            let theta = back!(Backtrack::from_result(
                Theta::from_lists(tparams, targs),
                span
            ));
            match &def_typ.node {
                ast::DefTypKind::Plain(typ) => {
                    let typ = back!(Backtrack::from_result(subst_typ(&theta, typ), span));
                    return cast_down(ctx, &typ, value);
                }
                _ => value,
            }
        }
        ast::TypKind::Tuple(typs) => {
            let values = back!(Backtrack::from_result(get::tuple(&value), span));
            if typs.len() != values.len() {
                return Backtrack::err(
                    span.clone(),
                    ErrorKind::Expr(ExprErrorKind::TupleCastArityMismatch {
                        expected: typs.len(),
                        actual: values.len(),
                    }),
                );
            }
            let mut values_cast = Vec::with_capacity(values.len());
            for (typ, value) in typs.iter().zip(values) {
                values_cast.push(back!(cast_down(ctx, typ, value.clone())));
            }
            make::tuple(typ, values_cast, Span::default())
        }
        ast::TypKind::Iter(typ_inner, ast::Iter::Opt) => {
            let value = back!(Backtrack::from_result(get::opt(&value), span));
            let value = match value {
                Some(value) => Some(back!(cast_down(ctx, typ_inner, value.clone()))),
                None => None,
            };
            make::opt(typ_inner, value, Span::default())
        }
        ast::TypKind::Iter(typ_inner, ast::Iter::List) => {
            let values = back!(Backtrack::from_result(get::list(&value), span));
            let mut values_cast = Vec::with_capacity(values.len());
            for value in values {
                values_cast.push(back!(cast_down(ctx, typ_inner, value.clone())));
            }
            make::list(typ_inner, values_cast, Span::default())
        }
        _ => value,
    };
    Backtrack::Ok(result)
}

// - Field access

pub(super) fn access_dot(value: &Value, atom: &ast::Atom, span: &Span) -> Backtrack<Rc<Value>> {
    let value_fields = back!(Backtrack::from_result(get::structure(value), span));
    match value_fields
        .iter()
        .find(|(field, _)| field.node == atom.node)
    {
        Some((_, value)) => Backtrack::Ok(value.clone()),
        None => Backtrack::err(
            atom.span.clone(),
            ErrorKind::Expr(ExprErrorKind::UndefinedField),
        ),
    }
}

// - Indices

fn get_index(value: &Value, span: &Span) -> Backtrack<i64> {
    let num = back!(Backtrack::from_result(get::num(value), span));
    let idx = num::to_int(num).to_i64();
    Backtrack::from_result(
        idx.ok_or(ErrorKind::Expr(ExprErrorKind::IndexOverflow)),
        span,
    )
}

// - Index access

pub(super) fn access_index(
    value_base: &Value,
    value_idx: &Value,
    span_base: &Span,
    span_idx: &Span,
) -> Backtrack<Rc<Value>> {
    let idx = back!(get_index(value_idx, span_idx));
    let len = match &value_base.node {
        ValueKind::Text(text) => text.len(),
        ValueKind::List(values) => values.len(),
        _ => {
            return Backtrack::err(
                span_base.clone(),
                ErrorKind::Expr(ExprErrorKind::IndexOperandMismatch),
            );
        }
    };
    if idx < 0 || idx as u64 >= len as u64 {
        return Backtrack::err(
            span_idx.clone(),
            ErrorKind::Expr(ExprErrorKind::IndexOutOfBounds { idx, len }),
        );
    }
    match &value_base.node {
        ValueKind::Text(_) => {
            let typ = crate::phrase!(node: value_base.note.clone(), span: value_base.span.clone());
            access_slice(
                value_base,
                value_idx,
                &make::nat(1u64.into(), Span::default()),
                &typ,
                span_base,
                span_idx,
                span_idx,
                span_idx,
            )
        }
        ValueKind::List(values) => Backtrack::Ok(values[idx as usize].clone()),
        _ => unreachable!(),
    }
}

// - Slice access

#[expect(
    clippy::too_many_arguments,
    reason = "operand and bounds spans remain explicit"
)]
pub(super) fn access_slice(
    value_base: &Value,
    value_idx: &Value,
    value_len: &Value,
    typ: &ast::Typ,
    span_base: &Span,
    span_idx: &Span,
    span_len: &Span,
    span_bounds: &Span,
) -> Backtrack<Rc<Value>> {
    let idx = back!(get_index(value_idx, span_idx));
    let len = back!(get_index(value_len, span_len));
    let end = back!(Backtrack::from_result(
        idx.checked_add(len)
            .ok_or(ErrorKind::Expr(ExprErrorKind::SliceEndOverflow)),
        span_bounds
    ));
    let size = match &value_base.node {
        ValueKind::Text(text) => text.len(),
        ValueKind::List(values) => values.len(),
        _ => {
            return Backtrack::err(
                span_base.clone(),
                ErrorKind::Expr(ExprErrorKind::SliceOperandMismatch),
            );
        }
    };
    if idx < 0 || end > size as i64 {
        return Backtrack::err(
            span_bounds.clone(),
            ErrorKind::Expr(ExprErrorKind::SliceOutOfBounds { idx, end, size }),
        );
    }
    match &value_base.node {
        ValueKind::Text(text) => {
            if len < 0 {
                return Backtrack::err(
                    span_bounds.clone(),
                    ErrorKind::Expr(ExprErrorKind::NegativeTextSliceLength),
                );
            }
            match text.get(idx as usize..end as usize) {
                Some(text) => Backtrack::Ok(make::text(text.to_owned(), Span::default())),
                None => Backtrack::err(
                    span_bounds.clone(),
                    ErrorKind::Expr(ExprErrorKind::TextSliceBoundaryMismatch),
                ),
            }
        }
        ValueKind::List(values) => {
            let values = values
                .iter()
                .enumerate()
                .filter(|(i, _)| idx <= *i as i64 && (*i as i64) < end)
                .map(|(_, value_base)| value_base.clone())
                .collect();
            Backtrack::Ok(make::list(typ, values, Span::default()))
        }
        _ => unreachable!(),
    }
}

// - Index update

pub(super) fn update_index(
    value_base: &Value,
    value_idx: &Value,
    value_upd: Rc<Value>,
    typ: &ast::Typ,
    span_base: &Span,
    span_idx: &Span,
) -> Backtrack<Rc<Value>> {
    let idx = back!(get_index(value_idx, span_idx));
    back!(access_index(value_base, value_idx, span_base, span_idx));
    let value = match &value_base.node {
        ValueKind::Text(text) => {
            let text_upd = back!(Backtrack::from_result(get::text(&value_upd), span_idx));
            if text_upd.len() != 1 {
                return Backtrack::err(
                    span_idx.clone(),
                    ErrorKind::Expr(ExprErrorKind::CharacterUpdateLengthMismatch),
                );
            }
            let value_l = back!(access_slice(
                value_base,
                &make::int((0).into(), Span::default()),
                &make::int((idx).into(), Span::default()),
                typ,
                span_base,
                span_idx,
                span_idx,
                span_idx
            ));
            let value_r = back!(access_slice(
                value_base,
                &make::int((idx + 1).into(), Span::default()),
                &make::int((text.len() as i64 - (idx + 1)).into(), Span::default()),
                typ,
                span_base,
                span_idx,
                span_idx,
                span_idx
            ));
            let text_l = back!(Backtrack::from_result(get::text(&value_l), span_idx));
            let text_r = back!(Backtrack::from_result(get::text(&value_r), span_idx));
            make::text(format!("{text_l}{text_upd}{text_r}"), Span::default())
        }
        ValueKind::List(values) => {
            let mut values = values.clone();
            values[idx as usize] = value_upd;
            make::list(typ, values, Span::default())
        }
        _ => unreachable!(),
    };
    Backtrack::Ok(value)
}

// - Slice update

#[expect(clippy::too_many_arguments, reason = "operand spans remain explicit")]
pub(super) fn update_slice(
    value_base: &Value,
    value_idx: &Value,
    value_len: &Value,
    value_upd: Rc<Value>,
    typ: &ast::Typ,
    span_base: &Span,
    span_idx: &Span,
    span_len: &Span,
) -> Backtrack<Rc<Value>> {
    let idx = back!(get_index(value_idx, span_idx));
    let len = back!(get_index(value_len, span_len));
    let end = back!(Backtrack::from_result(
        idx.checked_add(len)
            .ok_or(ErrorKind::Expr(ExprErrorKind::SliceEndOverflow)),
        span_len
    ));
    let size = match &value_base.node {
        ValueKind::Text(text) => text.len(),
        ValueKind::List(values) => values.len(),
        _ => {
            return Backtrack::err(
                span_base.clone(),
                ErrorKind::Expr(ExprErrorKind::SliceOperandMismatch),
            );
        }
    };
    if idx < 0 || end > size as i64 {
        return Backtrack::err(
            span_len.clone(),
            ErrorKind::Expr(ExprErrorKind::SliceOutOfBounds { idx, end, size }),
        );
    }
    let value = match &value_base.node {
        ValueKind::Text(text) => {
            let text_upd = back!(Backtrack::from_result(get::text(&value_upd), span_len));
            if len < 0 || text_upd.len() as i64 != len {
                return Backtrack::err(
                    span_len.clone(),
                    ErrorKind::Expr(ExprErrorKind::TextSliceUpdateLengthMismatch {
                        len,
                        actual: text_upd.len(),
                    }),
                );
            }
            let value_l = back!(access_slice(
                value_base,
                &make::int((0).into(), Span::default()),
                &make::int((idx).into(), Span::default()),
                typ,
                span_base,
                span_len,
                span_len,
                span_len
            ));
            let value_r = back!(access_slice(
                value_base,
                &make::int((end).into(), Span::default()),
                &make::int((text.len() as i64 - (end)).into(), Span::default()),
                typ,
                span_base,
                span_len,
                span_len,
                span_len
            ));
            let text_l = back!(Backtrack::from_result(get::text(&value_l), span_len));
            let text_r = back!(Backtrack::from_result(get::text(&value_r), span_len));
            make::text(format!("{text_l}{text_upd}{text_r}"), Span::default())
        }
        ValueKind::List(values) => {
            let values_upd = back!(Backtrack::from_result(get::list(&value_upd), span_len));
            if len < 0 || values_upd.len() as i64 != len {
                return Backtrack::err(
                    span_len.clone(),
                    ErrorKind::Expr(ExprErrorKind::ListSliceUpdateLengthMismatch {
                        len,
                        actual: values_upd.len(),
                    }),
                );
            }
            let mut values = values.clone();
            for (i, value) in values.iter_mut().enumerate() {
                if idx <= i as i64 && (i as i64) < end {
                    *value = values_upd[i - idx as usize].clone();
                }
            }
            make::list(typ, values, Span::default())
        }
        _ => unreachable!(),
    };
    Backtrack::Ok(value)
}
