//! Runtime type membership for executable values

use num_traits::Signed;
use thiserror::Error;

use crate::{
    lang::{
        common::source::Span,
        data::value::{Value, ValueArena, ValueKind},
        il::ast::{DefTypKind, FuncTyp, Iter, Subcheck, Typ, TypKind},
        xl::num::{Number, Typ as NumTyp},
    },
    runtime::{
        envs::elab::TDEnv,
        ops::typ::{Theta, TypeError, equiv_func_typ, subst_not_typ, subst_typ},
        typdef::TypeDef,
    },
};

// == Errors

#[derive(Clone, Debug, Error, PartialEq, Eq)]
pub enum MatchError {
    #[error("undefined type {name} at {span}")]
    UndefinedType { name: String, span: Span },

    #[error("unexpected type variable at {span}")]
    UnexpectedTypeVariable { span: Span },

    #[error("expected {expected} type arguments, got {actual} at {span}")]
    TypeArgumentMismatch {
        expected: usize,
        actual: usize,
        span: Span,
    },

    #[error("undefined function {name} at {span}")]
    UndefinedFunction { name: String, span: Span },

    #[error(transparent)]
    Type(#[from] TypeError),
}

// == Type membership

pub fn sub<F>(
    arena: &ValueArena,
    tdenv: &TDEnv,
    find_func: &F,
    typ: &Typ,
    value: &Value,
) -> Result<bool, MatchError>
where
    F: Fn(&str) -> Option<FuncTyp>,
{
    match &typ.node {
        TypKind::Bool => Ok(matches!(arena.kind(value), ValueKind::Bool(_))),
        TypKind::Num(NumTyp::Nat) => Ok(match arena.kind(value) {
            ValueKind::Num(Number::Nat(_)) => true,
            ValueKind::Num(Number::Int(integer)) => !integer.is_negative(),
            _ => false,
        }),
        TypKind::Num(NumTyp::Int) => Ok(matches!(arena.kind(value), ValueKind::Num(_))),
        TypKind::Text => Ok(matches!(arena.kind(value), ValueKind::Text(_))),
        TypKind::Var(id, targs) => {
            let type_def = tdenv.get(id).ok_or_else(|| MatchError::UndefinedType {
                name: id.node.clone(),
                span: typ.span.clone(),
            })?;
            match type_def {
                TypeDef::Parameter | TypeDef::Defining(_) => {
                    Err(MatchError::UnexpectedTypeVariable {
                        span: typ.span.clone(),
                    })
                }
                TypeDef::Extern => Ok(matches!(arena.kind(value), ValueKind::Extern(_))),
                TypeDef::Defined(tparams, def_typ) => {
                    let theta = Theta::from_lists(tparams, targs);
                    let theta = theta.map_err(|mismatch| MatchError::TypeArgumentMismatch {
                        expected: mismatch.expected,
                        actual: mismatch.actual,
                        span: typ.span.clone(),
                    })?;
                    match (&def_typ.node, arena.kind(value)) {
                        (DefTypKind::Plain(typ), _) => {
                            let typ = subst_typ(&theta, typ)?;
                            sub(arena, tdenv, find_func, &typ, value)
                        }
                        (DefTypKind::Struct(typ_fields), ValueKind::Struct(value_fields)) => {
                            if typ_fields.len() != value_fields.len() {
                                return Ok(false);
                            }
                            for ((typ_atom, typ), (value_atom, value)) in
                                typ_fields.iter().zip(value_fields)
                            {
                                if typ_atom.node != value_atom.node {
                                    return Ok(false);
                                }
                                let typ = subst_typ(&theta, typ)?;
                                if !sub(arena, tdenv, find_func, &typ, value)? {
                                    return Ok(false);
                                }
                            }
                            Ok(true)
                        }
                        (DefTypKind::Variant(typ_cases), ValueKind::Case(value_case)) => {
                            for (not_typ, _, _) in typ_cases {
                                if !not_typ.node.eq_shape(value_case) {
                                    continue;
                                }
                                let not_typ = subst_not_typ(&theta, not_typ)?;
                                let typs = not_typ.node.args();
                                let values = value_case.args();
                                if subs_inner(
                                    arena,
                                    tdenv,
                                    find_func,
                                    typs.into_iter(),
                                    values.into_iter(),
                                )? {
                                    return Ok(true);
                                }
                            }
                            Ok(false)
                        }
                        _ => Ok(false),
                    }
                }
            }
        }
        TypKind::Tuple(typs) => match arena.kind(value) {
            ValueKind::Tuple(values) => {
                subs_inner(arena, tdenv, find_func, typs.iter(), values.iter())
            }
            _ => Ok(false),
        },
        TypKind::Iter(typ_inner, Iter::Opt) => {
            if let ValueKind::Opt(Some(value)) = arena.kind(value) {
                sub(arena, tdenv, find_func, typ_inner, value)
            } else {
                Ok(true)
            }
        }
        TypKind::Iter(typ_inner, Iter::List) => match arena.kind(value) {
            ValueKind::List(values) => {
                for value in values {
                    if !sub(arena, tdenv, find_func, typ_inner, value)? {
                        return Ok(false);
                    }
                }
                Ok(true)
            }
            _ => Ok(false),
        },
        TypKind::Func(func_typ) => match arena.kind(value) {
            ValueKind::Func(id) => {
                let func_typ_actual =
                    find_func(&id.node).ok_or_else(|| MatchError::UndefinedFunction {
                        name: id.node.clone(),
                        span: id.span.clone(),
                    })?;
                let equivalent = equiv_func_typ(tdenv, &typ.span, func_typ, &func_typ_actual)?;
                Ok(equivalent)
            }
            _ => Ok(false),
        },
    }
}

pub fn subs<F>(
    arena: &ValueArena,
    tdenv: &TDEnv,
    find_func: &F,
    typs: &[Typ],
    values: &[Value],
) -> Result<bool, MatchError>
where
    F: Fn(&str) -> Option<FuncTyp>,
{
    subs_inner(arena, tdenv, find_func, typs.iter(), values.iter())
}

fn subs_inner<'typ, 'value, F, T, V>(
    arena: &ValueArena,
    tdenv: &TDEnv,
    find_func: &F,
    typs: T,
    values: V,
) -> Result<bool, MatchError>
where
    F: Fn(&str) -> Option<FuncTyp>,
    T: ExactSizeIterator<Item = &'typ Typ>,
    V: ExactSizeIterator<Item = &'value Value>,
{
    if typs.len() != values.len() {
        return Ok(false);
    }
    for (typ, value) in typs.zip(values) {
        if !sub(arena, tdenv, find_func, typ, value)? {
            return Ok(false);
        }
    }
    Ok(true)
}

// == Subtype-check execution

pub fn check<F>(
    arena: &ValueArena,
    tdenv: &TDEnv,
    find_func: &F,
    subcheck: &Subcheck,
    value: &Value,
) -> Result<bool, MatchError>
where
    F: Fn(&str) -> Option<FuncTyp>,
{
    match (subcheck, arena.kind(value)) {
        (Subcheck::Skip, _) => Ok(true),
        (Subcheck::Mixop(mixops), ValueKind::Case(value_case)) => {
            Ok(mixops.iter().any(|mixop| mixop.eq_shape(value_case)))
        }
        (Subcheck::Tuple(subchecks), ValueKind::Tuple(values)) => {
            if subchecks.len() != values.len() {
                return Ok(false);
            }
            for (subcheck, value) in subchecks.iter().zip(values) {
                if !check(arena, tdenv, find_func, subcheck, value)? {
                    return Ok(false);
                }
            }
            Ok(true)
        }
        (Subcheck::Iter(Iter::Opt, _), ValueKind::Opt(None)) => Ok(true),
        (Subcheck::Iter(Iter::Opt, subcheck), ValueKind::Opt(Some(value))) => {
            check(arena, tdenv, find_func, subcheck, value)
        }
        (Subcheck::Iter(Iter::List, subcheck), ValueKind::List(values)) => {
            for value in values {
                if !check(arena, tdenv, find_func, subcheck, value)? {
                    return Ok(false);
                }
            }
            Ok(true)
        }
        (Subcheck::Recurse(typ), _) => sub(arena, tdenv, find_func, typ, value),
        _ => Ok(false),
    }
}
