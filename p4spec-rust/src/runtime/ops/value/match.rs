//! Runtime type membership for executable values

use std::rc::Rc;

use num_traits::Signed;
use thiserror::Error;

use crate::{
    lang::{
        common::source::Span,
        data::value::{Value, ValueKind},
        il::ast::{DefTypKind, FuncTyp, Iter, Subcheck, Typ, TypKind},
        xl::num::{Number, Typ as NumTyp},
    },
    runtime::{
        env::TDEnv,
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

fn substitution(
    tparams: &[crate::lang::il::ast::TParam],
    targs: &[crate::lang::il::ast::Targ],
    span: &Span,
) -> Result<Theta, MatchError> {
    if tparams.len() != targs.len() {
        return Err(MatchError::TypeArgumentMismatch {
            expected: tparams.len(),
            actual: targs.len(),
            span: span.clone(),
        });
    }
    let mut theta = Theta::new();
    for (tparam, targ) in tparams.iter().zip(targs) {
        theta.insert(tparam.clone(), targ.clone());
    }
    Ok(theta)
}

pub fn sub<F>(tdenv: &TDEnv, find_func: &F, typ: &Typ, value: &Value) -> Result<bool, MatchError>
where
    F: Fn(&str) -> Option<FuncTyp>,
{
    match &typ.node {
        TypKind::Bool => Ok(matches!(value.node, ValueKind::Bool(_))),
        TypKind::Num(NumTyp::Nat) => Ok(match &value.node {
            ValueKind::Num(Number::Nat(_)) => true,
            ValueKind::Num(Number::Int(integer)) => !integer.is_negative(),
            _ => false,
        }),
        TypKind::Num(NumTyp::Int) => Ok(matches!(value.node, ValueKind::Num(_))),
        TypKind::Text => Ok(matches!(value.node, ValueKind::Text(_))),
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
                TypeDef::Extern => Ok(matches!(value.node, ValueKind::Extern(_))),
                TypeDef::Defined(tparams, def_typ) => {
                    let theta = substitution(tparams, targs, &typ.span)?;
                    match (&def_typ.node, &value.node) {
                        (DefTypKind::Plain(typ), _) => {
                            let typ = subst_typ(&theta, typ)?;
                            sub(tdenv, find_func, &typ, value)
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
                                if !sub(tdenv, find_func, &typ, value)? {
                                    return Ok(false);
                                }
                            }
                            Ok(true)
                        }
                        (DefTypKind::Variant(typ_cases), ValueKind::Case(value_case)) => {
                            for (not_typ, _, _) in typ_cases {
                                if not_typ.node.to_mixop() != value_case.to_mixop() {
                                    continue;
                                }
                                let not_typ = subst_not_typ(&theta, not_typ)?;
                                let typs = not_typ.node.args();
                                let values = value_case.args();
                                if subs_inner(
                                    tdenv,
                                    find_func,
                                    typs.into_iter(),
                                    values.into_iter().map(AsRef::as_ref),
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
        TypKind::Tuple(typs) => match &value.node {
            ValueKind::Tuple(values) => subs_inner(
                tdenv,
                find_func,
                typs.iter(),
                values.iter().map(AsRef::as_ref),
            ),
            _ => Ok(false),
        },
        TypKind::Iter(typ_inner, Iter::Opt) => {
            if let ValueKind::Opt(Some(value)) = &value.node {
                sub(tdenv, find_func, typ_inner, value)
            } else {
                Ok(true)
            }
        }
        TypKind::Iter(typ_inner, Iter::List) => match &value.node {
            ValueKind::List(values) => {
                for value in values {
                    if !sub(tdenv, find_func, typ_inner, value)? {
                        return Ok(false);
                    }
                }
                Ok(true)
            }
            _ => Ok(false),
        },
        TypKind::Func(func_typ) => match &value.node {
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

fn subs_inner<'typ, 'value, F, T, V>(
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
        if !sub(tdenv, find_func, typ, value)? {
            return Ok(false);
        }
    }
    Ok(true)
}

pub fn subs<F>(
    tdenv: &TDEnv,
    find_func: &F,
    typs: &[Typ],
    values: &[Rc<Value>],
) -> Result<bool, MatchError>
where
    F: Fn(&str) -> Option<FuncTyp>,
{
    subs_inner(
        tdenv,
        find_func,
        typs.iter(),
        values.iter().map(AsRef::as_ref),
    )
}

// == Subtype-check execution

pub fn check<F>(
    tdenv: &TDEnv,
    find_func: &F,
    subcheck: &Subcheck,
    value: &Value,
) -> Result<bool, MatchError>
where
    F: Fn(&str) -> Option<FuncTyp>,
{
    match (subcheck, &value.node) {
        (Subcheck::Skip, _) => Ok(true),
        (Subcheck::Mixop(mixops), ValueKind::Case(value_case)) => {
            Ok(mixops.iter().any(|mixop| mixop == &value_case.to_mixop()))
        }
        (Subcheck::Tuple(subchecks), ValueKind::Tuple(values)) => {
            if subchecks.len() != values.len() {
                return Ok(false);
            }
            for (subcheck, value) in subchecks.iter().zip(values) {
                if !check(tdenv, find_func, subcheck, value)? {
                    return Ok(false);
                }
            }
            Ok(true)
        }
        (Subcheck::Iter(Iter::Opt, _), ValueKind::Opt(None)) => Ok(true),
        (Subcheck::Iter(Iter::Opt, subcheck), ValueKind::Opt(Some(value))) => {
            check(tdenv, find_func, subcheck, value)
        }
        (Subcheck::Iter(Iter::List, subcheck), ValueKind::List(values)) => {
            for value in values {
                if !check(tdenv, find_func, subcheck, value)? {
                    return Ok(false);
                }
            }
            Ok(true)
        }
        (Subcheck::Recurse(typ), _) => sub(tdenv, find_func, typ, value),
        _ => Ok(false),
    }
}
