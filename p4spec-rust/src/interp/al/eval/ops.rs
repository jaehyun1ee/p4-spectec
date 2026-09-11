//! Value operations shared by AL expression and path evaluation

use num_traits::ToPrimitive;

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
    backtrack::{Backtrack, back},
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
            let num = back!(Backtrack::from_result(get::num(arena, &value), span));
            match num {
                num::Number::Nat(num) => {
                    let num = num.as_bigint().clone();
                    back!(Backtrack::from_result(
                        make::int(arena, num, Span::default()),
                        span
                    ))
                }
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
                    return cast_up(arena, ctx, &typ, value);
                }
                _ => value,
            }
        }
        ast::TypKind::Tuple(typs) => {
            let values = back!(Backtrack::from_result(get::tuple(arena, &value), span)).to_vec();
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
                values_cast.push(back!(cast_up(arena, ctx, typ, value)));
            }
            back!(Backtrack::from_result(
                make::tuple(arena, typ.node.clone().into(), values_cast, Span::default()),
                span
            ))
        }
        ast::TypKind::Iter(typ_inner, ast::Iter::Opt) => {
            let value = back!(Backtrack::from_result(get::opt(arena, &value), span));
            let value = match value {
                Some(value) => Some(back!(cast_up(arena, ctx, typ_inner, value))),
                None => None,
            };
            back!(Backtrack::from_result(
                make::opt(arena, typ_inner.node.clone().into(), value, Span::default()),
                span
            ))
        }
        ast::TypKind::Iter(typ_inner, ast::Iter::List) => {
            let values = back!(Backtrack::from_result(get::list(arena, &value), span)).to_vec();
            let mut values_cast = Vec::with_capacity(values.len());
            for value in values {
                values_cast.push(back!(cast_up(arena, ctx, typ_inner, value)));
            }
            back!(Backtrack::from_result(
                make::list(
                    arena,
                    typ_inner.node.clone().into(),
                    values_cast,
                    Span::default()
                ),
                span
            ))
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
            let num = back!(Backtrack::from_result(get::num(arena, &value), span));
            match num {
                num::Number::Nat(_) => value,
                num::Number::Int(num) => {
                    let num = back!(Backtrack::from_result(
                        num::Natural::try_from(num.clone()),
                        span
                    ));
                    back!(Backtrack::from_result(
                        make::nat(arena, num, Span::default()),
                        span
                    ))
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
                    return cast_down(arena, ctx, &typ, value);
                }
                _ => value,
            }
        }
        ast::TypKind::Tuple(typs) => {
            let values = back!(Backtrack::from_result(get::tuple(arena, &value), span)).to_vec();
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
                values_cast.push(back!(cast_down(arena, ctx, typ, value)));
            }
            back!(Backtrack::from_result(
                make::tuple(arena, typ.node.clone().into(), values_cast, Span::default()),
                span
            ))
        }
        ast::TypKind::Iter(typ_inner, ast::Iter::Opt) => {
            let value = back!(Backtrack::from_result(get::opt(arena, &value), span));
            let value = match value {
                Some(value) => Some(back!(cast_down(arena, ctx, typ_inner, value))),
                None => None,
            };
            back!(Backtrack::from_result(
                make::opt(arena, typ_inner.node.clone().into(), value, Span::default()),
                span
            ))
        }
        ast::TypKind::Iter(typ_inner, ast::Iter::List) => {
            let values = back!(Backtrack::from_result(get::list(arena, &value), span)).to_vec();
            let mut values_cast = Vec::with_capacity(values.len());
            for value in values {
                values_cast.push(back!(cast_down(arena, ctx, typ_inner, value)));
            }
            back!(Backtrack::from_result(
                make::list(
                    arena,
                    typ_inner.node.clone().into(),
                    values_cast,
                    Span::default()
                ),
                span
            ))
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
    let value_fields = back!(Backtrack::from_result(get::structure(arena, value), span));
    match value_fields
        .iter()
        .find(|(field, _)| field.node == atom.node)
    {
        Some((_, value)) => Backtrack::Ok(*value),
        None => Backtrack::err(
            atom.span.clone(),
            ErrorKind::Expr(ExprErrorKind::UndefinedField),
        ),
    }
}

// - Indices

fn get_index(arena: &ValueArena, value: &Value, span: &Span) -> Backtrack<i64> {
    let num = back!(Backtrack::from_result(get::num(arena, value), span));
    let idx = num::to_int(num).to_i64();
    Backtrack::from_result(
        idx.ok_or(ErrorKind::Expr(ExprErrorKind::IndexOverflow)),
        span,
    )
}

// - Index access

pub(super) fn access_index(
    arena: &mut ValueArena,
    value_base: &Value,
    value_idx: &Value,
    span_base: &Span,
    span_idx: &Span,
) -> Backtrack<Value> {
    let idx = back!(get_index(arena, value_idx, span_idx));
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
    if idx < 0 || idx as u64 >= len as u64 {
        return Backtrack::err(
            span_idx.clone(),
            ErrorKind::Expr(ExprErrorKind::IndexOutOfBounds { idx, len }),
        );
    }
    match arena.kind(value_base) {
        ValueKind::Text(_) => {
            let typ = crate::phrase!(node: arena.typ(value_base).clone(), span: arena.span(value_base).clone());
            let value_len = back!(Backtrack::from_result(
                make::nat(arena, 1u64.into(), Span::default()),
                span_idx
            ));
            access_slice(
                arena, value_base, value_idx, &value_len, &typ.node, &typ.span, span_base,
                span_idx, span_idx, span_idx,
            )
        }
        ValueKind::List(values) => Backtrack::Ok(values[idx as usize]),
        _ => unreachable!(),
    }
}

// - Slice access

#[expect(
    clippy::too_many_arguments,
    reason = "operand and bounds spans remain explicit"
)]
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
    let idx = back!(get_index(arena, value_idx, span_idx));
    let len = back!(get_index(arena, value_len, span_len));
    let end = back!(Backtrack::from_result(
        idx.checked_add(len)
            .ok_or(ErrorKind::Expr(ExprErrorKind::SliceEndOverflow)),
        span_bounds
    ));
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
    if idx < 0 || end > size as i64 {
        return Backtrack::err(
            span_bounds.clone(),
            ErrorKind::Expr(ExprErrorKind::SliceOutOfBounds { idx, end, size }),
        );
    }
    match arena.kind(value_base) {
        ValueKind::Text(text) => {
            if len < 0 {
                return Backtrack::err(
                    span_bounds.clone(),
                    ErrorKind::Expr(ExprErrorKind::NegativeTextSliceLength),
                );
            }
            match text.get(idx as usize..end as usize) {
                Some(text) => {
                    let text = text.to_owned();
                    Backtrack::from_result(make::text(arena, text, Span::default()), span_typ)
                }
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
                .map(|(_, value_base)| *value_base)
                .collect();
            Backtrack::Ok(back!(Backtrack::from_result(
                make::list(arena, typ.clone(), values, Span::default()),
                span_typ
            )))
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
    let idx = back!(get_index(arena, value_idx, span_idx));
    back!(access_index(
        arena, value_base, value_idx, span_base, span_idx
    ));
    let value = match arena.kind(value_base) {
        ValueKind::Text(text) => {
            let size = text.len() as i64;
            let text_upd = back!(Backtrack::from_result(
                get::text(arena, &value_upd),
                span_idx
            ));
            if text_upd.len() != 1 {
                return Backtrack::err(
                    span_idx.clone(),
                    ErrorKind::Expr(ExprErrorKind::CharacterUpdateLengthMismatch),
                );
            }
            let text_upd = text_upd.to_owned();
            let value_l_idx = back!(Backtrack::from_result(
                make::int(arena, (0).into(), Span::default()),
                &typ.span
            ));
            let value_l_len = back!(Backtrack::from_result(
                make::int(arena, (idx).into(), Span::default()),
                &typ.span
            ));
            let value_l = back!(access_slice(
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
            let value_r_idx = back!(Backtrack::from_result(
                make::int(arena, (idx + 1).into(), Span::default()),
                &typ.span
            ));
            let value_r_len = back!(Backtrack::from_result(
                make::int(arena, (size - (idx + 1)).into(), Span::default()),
                &typ.span
            ));
            let value_r = back!(access_slice(
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
            let text_l = back!(Backtrack::from_result(get::text(arena, &value_l), span_idx));
            let text_r = back!(Backtrack::from_result(get::text(arena, &value_r), span_idx));
            {
                let text = format!("{text_l}{text_upd}{text_r}");
                back!(Backtrack::from_result(
                    make::text(arena, text, Span::default()),
                    &typ.span
                ))
            }
        }
        ValueKind::List(values) => {
            let mut values = values.clone();
            values[idx as usize] = value_upd;
            back!(Backtrack::from_result(
                make::list(arena, typ.node.clone(), values, Span::default()),
                &typ.span
            ))
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
    let idx = back!(get_index(arena, value_idx, span_idx));
    let len = back!(get_index(arena, value_len, span_len));
    let end = back!(Backtrack::from_result(
        idx.checked_add(len)
            .ok_or(ErrorKind::Expr(ExprErrorKind::SliceEndOverflow)),
        span_len
    ));
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
    if idx < 0 || end > size as i64 {
        return Backtrack::err(
            span_len.clone(),
            ErrorKind::Expr(ExprErrorKind::SliceOutOfBounds { idx, end, size }),
        );
    }
    let value = match arena.kind(value_base) {
        ValueKind::Text(text) => {
            let size = text.len() as i64;
            let text_upd = back!(Backtrack::from_result(
                get::text(arena, &value_upd),
                span_len
            ));
            if len < 0 || text_upd.len() as i64 != len {
                return Backtrack::err(
                    span_len.clone(),
                    ErrorKind::Expr(ExprErrorKind::TextSliceUpdateLengthMismatch {
                        len,
                        actual: text_upd.len(),
                    }),
                );
            }
            let text_upd = text_upd.to_owned();
            let value_l_idx = back!(Backtrack::from_result(
                make::int(arena, (0).into(), Span::default()),
                &typ.span
            ));
            let value_l_len = back!(Backtrack::from_result(
                make::int(arena, (idx).into(), Span::default()),
                &typ.span
            ));
            let value_l = back!(access_slice(
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
            let value_r_idx = back!(Backtrack::from_result(
                make::int(arena, (end).into(), Span::default()),
                &typ.span
            ));
            let value_r_len = back!(Backtrack::from_result(
                make::int(arena, (size - (end)).into(), Span::default()),
                &typ.span
            ));
            let value_r = back!(access_slice(
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
            let text_l = back!(Backtrack::from_result(get::text(arena, &value_l), span_len));
            let text_r = back!(Backtrack::from_result(get::text(arena, &value_r), span_len));
            {
                let text = format!("{text_l}{text_upd}{text_r}");
                back!(Backtrack::from_result(
                    make::text(arena, text, Span::default()),
                    &typ.span
                ))
            }
        }
        ValueKind::List(values) => {
            let values_upd = back!(Backtrack::from_result(
                get::list(arena, &value_upd),
                span_len
            ));
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
                    *value = values_upd[i - idx as usize];
                }
            }
            back!(Backtrack::from_result(
                make::list(arena, typ.node.clone(), values, Span::default()),
                &typ.span
            ))
        }
        _ => unreachable!(),
    };
    Backtrack::Ok(value)
}
