//! Value operations shared by AL expression and path evaluation

use num_bigint::BigInt;

use std::rc::Rc;

use crate::{
    lang::{
        al::ast,
        common::source::{Phrase, Span},
        data::value::{Value, ValueArena, ValueKind, get, make},
        xl::num,
    },
    runtime::ops::typ::{Theta, subst_typ},
};

use super::super::{
    backtrack::{Backtrack, backtrack, backtrack_from_result},
    context::Context,
    error::{ErrorKind, ExprErrorKind},
};

// = Value operations

// - Upcast

pub(super) fn cast_up(
    arena: &mut ValueArena,
    ctx: &Context<'_>,
    typ: &ast::Typ,
    value: Value,
) -> Backtrack<Value> {
    let span = &typ.span;
    let result = match &typ.node {
        ast::TypKind::Num(num::Typ::Int) => {
            let num = backtrack_from_result!(get::num(arena, &value), span);
            match num {
                num::Number::Nat(num) => {
                    let num = num.as_bigint().clone();
                    backtrack_from_result!(make::int(arena, num, Span::default()), span)
                }
                num::Number::Int(_) => value,
            }
        }
        ast::TypKind::Var(id, targs) => {
            let (tparams, def_typ) = backtrack_from_result!(ctx.find_defined_typdef(id), span);
            let theta = backtrack_from_result!(Theta::from_lists(tparams, targs), span);
            match &def_typ.node {
                ast::DefTypKind::Plain(typ) => {
                    let typ = backtrack_from_result!(subst_typ(&theta, typ), span);
                    return cast_up(arena, ctx, &typ, value);
                }
                _ => value,
            }
        }
        ast::TypKind::Tuple(typs) => {
            let values = backtrack_from_result!(get::tuple(arena, &value), span).to_vec();
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
                values_cast.push(backtrack!(cast_up(arena, ctx, typ, value)));
            }
            backtrack_from_result!(
                make::tuple(arena, typ.node.clone().into(), values_cast, Span::default()),
                span
            )
        }
        ast::TypKind::Iter(typ_inner, ast::Iter::Opt) => {
            let value = backtrack_from_result!(get::opt(arena, &value), span);
            let value = match value {
                Some(value) => Some(backtrack!(cast_up(arena, ctx, typ_inner, value))),
                None => None,
            };
            backtrack_from_result!(
                make::opt(arena, typ_inner.node.clone().into(), value, Span::default()),
                span
            )
        }
        ast::TypKind::Iter(typ_inner, ast::Iter::List) => {
            let values = backtrack_from_result!(get::list(arena, &value), span).to_vec();
            let mut values_cast = Vec::with_capacity(values.len());
            for value in values {
                values_cast.push(backtrack!(cast_up(arena, ctx, typ_inner, value)));
            }
            backtrack_from_result!(
                make::list(arena, typ_inner.node.clone().into(), values_cast, Span::default()),
                span
            )
        }
        _ => value,
    };
    Backtrack::Ok(result)
}

// - Downcast

pub(super) fn cast_down(
    arena: &mut ValueArena,
    ctx: &Context<'_>,
    typ: &ast::Typ,
    value: Value,
) -> Backtrack<Value> {
    let span = &typ.span;
    let result = match &typ.node {
        ast::TypKind::Num(num::Typ::Nat) => {
            let num = backtrack_from_result!(get::num(arena, &value), span);
            match num {
                num::Number::Nat(_) => value,
                num::Number::Int(num) => {
                    let num = backtrack_from_result!(num::Natural::try_from(num.clone()), span);
                    backtrack_from_result!(make::nat(arena, num, Span::default()), span)
                }
            }
        }
        ast::TypKind::Var(id, targs) => {
            let (tparams, def_typ) = backtrack_from_result!(ctx.find_defined_typdef(id), span);
            let theta = backtrack_from_result!(Theta::from_lists(tparams, targs), span);
            match &def_typ.node {
                ast::DefTypKind::Plain(typ) => {
                    let typ = backtrack_from_result!(subst_typ(&theta, typ), span);
                    return cast_down(arena, ctx, &typ, value);
                }
                _ => value,
            }
        }
        ast::TypKind::Tuple(typs) => {
            let values = backtrack_from_result!(get::tuple(arena, &value), span).to_vec();
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
                values_cast.push(backtrack!(cast_down(arena, ctx, typ, value)));
            }
            backtrack_from_result!(
                make::tuple(arena, typ.node.clone().into(), values_cast, Span::default()),
                span
            )
        }
        ast::TypKind::Iter(typ_inner, ast::Iter::Opt) => {
            let value = backtrack_from_result!(get::opt(arena, &value), span);
            let value = match value {
                Some(value) => Some(backtrack!(cast_down(arena, ctx, typ_inner, value))),
                None => None,
            };
            backtrack_from_result!(
                make::opt(arena, typ_inner.node.clone().into(), value, Span::default()),
                span
            )
        }
        ast::TypKind::Iter(typ_inner, ast::Iter::List) => {
            let values = backtrack_from_result!(get::list(arena, &value), span).to_vec();
            let mut values_cast = Vec::with_capacity(values.len());
            for value in values {
                values_cast.push(backtrack!(cast_down(arena, ctx, typ_inner, value)));
            }
            backtrack_from_result!(
                make::list(arena, typ_inner.node.clone().into(), values_cast, Span::default()),
                span
            )
        }
        _ => value,
    };
    Backtrack::Ok(result)
}

// - Field access

pub(super) fn access_dot(
    arena: &ValueArena,
    value: &Value,
    atom: &ast::Atom,
    span: &Span,
) -> Backtrack<Value> {
    let value_fields = backtrack_from_result!(get::structure(arena, value), span);
    match value_fields
        .iter()
        .find(|(field, _)| field.node == atom.node)
    {
        Some((_, value)) => Backtrack::Ok(*value),
        None => Backtrack::err(atom.span.clone(), ErrorKind::Expr(ExprErrorKind::UndefinedField)),
    }
}

// - Indices

fn get_int(arena: &ValueArena, value: &Value, span: &Span) -> Backtrack<BigInt> {
    let num = backtrack_from_result!(get::num(arena, value), span);
    Backtrack::Ok(num::to_int(num).clone())
}

// - Index access

pub(super) fn access_index(
    arena: &mut ValueArena,
    value_base: &Value,
    value_idx: &Value,
    span_base: &Span,
    span_idx: &Span,
) -> Backtrack<Value> {
    let int_idx = backtrack!(get_int(arena, value_idx, span_idx));
    let len = match arena.kind(value_base) {
        ValueKind::Text(text) => text.len(),
        ValueKind::List(values) => values.len(),
        _ => {
            return Backtrack::err(
                span_base.clone(),
                ErrorKind::Expr(ExprErrorKind::IndexOperandMismatch),
            );
        }
    };
    let Some(idx) = usize::try_from(&int_idx).ok().filter(|idx| *idx < len) else {
        return Backtrack::err(
            span_idx.clone(),
            ErrorKind::Expr(ExprErrorKind::IndexOutOfBounds { idx: int_idx, len }),
        );
    };
    match arena.kind(value_base) {
        ValueKind::Text(_) => {
            let typ = crate::phrase!(node: arena.typ(value_base).clone(), span: arena.span(value_base).clone());
            let value_len =
                backtrack_from_result!(make::nat(arena, 1u64.into(), Span::default()), span_idx);
            access_slice(
                arena, value_base, value_idx, &value_len, &typ.node, &typ.span, span_base,
                span_idx, span_idx, span_idx,
            )
        }
        ValueKind::List(values) => Backtrack::Ok(values[idx]),
        _ => unreachable!(),
    }
}

// - Slice access

#[expect(clippy::too_many_arguments, reason = "operand and bounds spans remain explicit")]
pub(super) fn access_slice(
    arena: &mut ValueArena,
    value_base: &Value,
    value_idx: &Value,
    value_len: &Value,
    typ: &Rc<ast::TypKind>,
    span_typ: &Span,
    span_base: &Span,
    span_idx: &Span,
    span_len: &Span,
    span_bounds: &Span,
) -> Backtrack<Value> {
    let int_idx = backtrack!(get_int(arena, value_idx, span_idx));
    let int_len = backtrack!(get_int(arena, value_len, span_len));
    let size = match arena.kind(value_base) {
        ValueKind::Text(text) => text.len(),
        ValueKind::List(values) => values.len(),
        _ => {
            return Backtrack::err(
                span_base.clone(),
                ErrorKind::Expr(ExprErrorKind::SliceOperandMismatch),
            );
        }
    };
    let Some((idx, idx_end)) = usize::try_from(&int_idx)
        .ok()
        .zip(usize::try_from(&int_len).ok())
        .and_then(|(idx, len)| idx.checked_add(len).map(|idx_end| (idx, idx_end)))
        .filter(|(_, idx_end)| *idx_end <= size)
    else {
        let int_end = &int_idx + &int_len;
        return Backtrack::err(
            span_bounds.clone(),
            ErrorKind::Expr(ExprErrorKind::SliceOutOfBounds { idx: int_idx, end: int_end, size }),
        );
    };
    match arena.kind(value_base) {
        ValueKind::Text(text) => match text.get(idx..idx_end) {
            Some(text) => {
                let text = text.to_owned();
                Backtrack::from_result(make::text(arena, text, Span::default()), span_typ)
            }
            None => Backtrack::err(
                span_bounds.clone(),
                ErrorKind::Expr(ExprErrorKind::TextSliceBoundaryMismatch),
            ),
        },
        ValueKind::List(values) => {
            let values = values[idx..idx_end].to_vec();
            Backtrack::Ok(backtrack_from_result!(
                make::list(arena, typ.clone(), values, Span::default()),
                span_typ
            ))
        }
        _ => unreachable!(),
    }
}

// - Index update

pub(super) fn update_index(
    arena: &mut ValueArena,
    value_base: &Value,
    value_idx: &Value,
    value_upd: Value,
    typ: &Phrase<Rc<ast::TypKind>>,
    span_base: &Span,
    span_idx: &Span,
) -> Backtrack<Value> {
    let int_idx = backtrack!(get_int(arena, value_idx, span_idx));
    let len = match arena.kind(value_base) {
        ValueKind::Text(text) => text.len(),
        ValueKind::List(values) => values.len(),
        _ => {
            return Backtrack::err(
                span_base.clone(),
                ErrorKind::Expr(ExprErrorKind::IndexOperandMismatch),
            );
        }
    };
    let Some(idx) = usize::try_from(&int_idx).ok().filter(|idx| *idx < len) else {
        return Backtrack::err(
            span_idx.clone(),
            ErrorKind::Expr(ExprErrorKind::IndexOutOfBounds { idx: int_idx, len }),
        );
    };
    let value = match arena.kind(value_base) {
        ValueKind::Text(text) => {
            let size = text.len();
            let text_upd = backtrack_from_result!(get::text(arena, &value_upd), span_idx);
            if text_upd.len() != 1 {
                return Backtrack::err(
                    span_idx.clone(),
                    ErrorKind::Expr(ExprErrorKind::CharacterUpdateLengthMismatch),
                );
            }
            let text_upd = text_upd.to_owned();
            let value_l_idx =
                backtrack_from_result!(make::int(arena, (0).into(), Span::default()), &typ.span);
            let value_l_len =
                backtrack_from_result!(make::int(arena, (idx).into(), Span::default()), &typ.span);
            let value_l = backtrack!(access_slice(
                arena,
                value_base,
                &value_l_idx,
                &value_l_len,
                &typ.node,
                &typ.span,
                span_base,
                span_idx,
                span_idx,
                span_idx
            ));
            let value_r_idx = backtrack_from_result!(
                make::int(arena, (idx + 1).into(), Span::default()),
                &typ.span
            );
            let value_r_len = backtrack_from_result!(
                make::int(arena, (size - (idx + 1)).into(), Span::default()),
                &typ.span
            );
            let value_r = backtrack!(access_slice(
                arena,
                value_base,
                &value_r_idx,
                &value_r_len,
                &typ.node,
                &typ.span,
                span_base,
                span_idx,
                span_idx,
                span_idx
            ));
            let text_l = backtrack_from_result!(get::text(arena, &value_l), span_idx);
            let text_r = backtrack_from_result!(get::text(arena, &value_r), span_idx);
            {
                let text = format!("{text_l}{text_upd}{text_r}");
                backtrack_from_result!(make::text(arena, text, Span::default()), &typ.span)
            }
        }
        ValueKind::List(values) => {
            let mut values = values.clone();
            values[idx] = value_upd;
            backtrack_from_result!(
                make::list(arena, typ.node.clone(), values, Span::default()),
                &typ.span
            )
        }
        _ => unreachable!(),
    };
    Backtrack::Ok(value)
}

// - Slice update

#[expect(clippy::too_many_arguments, reason = "operand spans remain explicit")]
pub(super) fn update_slice(
    arena: &mut ValueArena,
    value_base: &Value,
    value_idx: &Value,
    value_len: &Value,
    value_upd: Value,
    typ: &Phrase<Rc<ast::TypKind>>,
    span_base: &Span,
    span_idx: &Span,
    span_len: &Span,
) -> Backtrack<Value> {
    let int_idx = backtrack!(get_int(arena, value_idx, span_idx));
    let int_len = backtrack!(get_int(arena, value_len, span_len));
    let size = match arena.kind(value_base) {
        ValueKind::Text(text) => text.len(),
        ValueKind::List(values) => values.len(),
        _ => {
            return Backtrack::err(
                span_base.clone(),
                ErrorKind::Expr(ExprErrorKind::SliceOperandMismatch),
            );
        }
    };
    let Some((idx, idx_end)) = usize::try_from(&int_idx)
        .ok()
        .zip(usize::try_from(&int_len).ok())
        .and_then(|(idx, len)| idx.checked_add(len).map(|idx_end| (idx, idx_end)))
        .filter(|(_, idx_end)| *idx_end <= size)
    else {
        let int_end = &int_idx + &int_len;
        return Backtrack::err(
            span_len.clone(),
            ErrorKind::Expr(ExprErrorKind::SliceOutOfBounds { idx: int_idx, end: int_end, size }),
        );
    };
    let value = match arena.kind(value_base) {
        ValueKind::Text(text) => {
            let size = text.len();
            let text_upd = backtrack_from_result!(get::text(arena, &value_upd), span_len);
            if text_upd.len() != idx_end - idx {
                return Backtrack::err(
                    span_len.clone(),
                    ErrorKind::Expr(ExprErrorKind::TextSliceUpdateLengthMismatch {
                        len: idx_end - idx,
                        actual: text_upd.len(),
                    }),
                );
            }
            let text_upd = text_upd.to_owned();
            let value_l_idx =
                backtrack_from_result!(make::int(arena, (0).into(), Span::default()), &typ.span);
            let value_l_len =
                backtrack_from_result!(make::int(arena, (idx).into(), Span::default()), &typ.span);
            let value_l = backtrack!(access_slice(
                arena,
                value_base,
                &value_l_idx,
                &value_l_len,
                &typ.node,
                &typ.span,
                span_base,
                span_len,
                span_len,
                span_len
            ));
            let value_r_idx = backtrack_from_result!(
                make::int(arena, (idx_end).into(), Span::default()),
                &typ.span
            );
            let value_r_len = backtrack_from_result!(
                make::int(arena, (size - (idx_end)).into(), Span::default()),
                &typ.span
            );
            let value_r = backtrack!(access_slice(
                arena,
                value_base,
                &value_r_idx,
                &value_r_len,
                &typ.node,
                &typ.span,
                span_base,
                span_len,
                span_len,
                span_len
            ));
            let text_l = backtrack_from_result!(get::text(arena, &value_l), span_len);
            let text_r = backtrack_from_result!(get::text(arena, &value_r), span_len);
            {
                let text = format!("{text_l}{text_upd}{text_r}");
                backtrack_from_result!(make::text(arena, text, Span::default()), &typ.span)
            }
        }
        ValueKind::List(values) => {
            let values_upd = backtrack_from_result!(get::list(arena, &value_upd), span_len);
            if values_upd.len() != idx_end - idx {
                return Backtrack::err(
                    span_len.clone(),
                    ErrorKind::Expr(ExprErrorKind::ListSliceUpdateLengthMismatch {
                        len: idx_end - idx,
                        actual: values_upd.len(),
                    }),
                );
            }
            let mut values = values.clone();
            values[idx..idx_end].clone_from_slice(values_upd);
            backtrack_from_result!(
                make::list(arena, typ.node.clone(), values, Span::default()),
                &typ.span
            )
        }
        _ => unreachable!(),
    };
    Backtrack::Ok(value)
}
