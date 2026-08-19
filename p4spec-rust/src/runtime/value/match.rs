use std::{collections::HashMap, rc::Rc};

use num_bigint::Sign;
use thiserror::Error;

use crate::{
    domain::source::Region,
    lang::{
        il::ast::{DefTypKind, Iter, TParam, Typ, TypKind},
        xl::num,
    },
    runtime::r#type::{
        envs::TypeDefMap,
        equiv::{self, EquivError},
        subst::{self, SubstError, TypeSubstitution},
        typdef::TypeDef,
    },
};

use super::{Value, ValueKind, ValueRef};

#[derive(Clone, Debug)]
pub struct FuncSignature {
    pub type_params: Vec<TParam>,
    pub param_types: Vec<Typ>,
    pub return_type: Typ,
}

#[derive(Clone, Debug, Error, PartialEq)]
pub enum MatchError {
    #[error("undefined type {name} at {span}")]
    UndefinedType { name: String, span: Region },
    #[error("unexpected type variable at {span}")]
    UnexpectedTypeVariable { span: Region },
    #[error("expected {expected} type arguments, got {actual} at {span}")]
    TypeArgumentMismatch {
        expected: usize,
        actual: usize,
        span: Region,
    },
    #[error("undefined function {name} at {span}")]
    UndefinedFunction { name: String, span: Region },
    #[error(transparent)]
    Equivalence(#[from] EquivError),
    #[error(transparent)]
    Substitution(#[from] SubstError),
}

// Whether a value belongs to a type (including subtyping)

fn sub_inner<F>(
    type_defs: &TypeDefMap,
    find_func: &F,
    typ: &Typ,
    value: &Value,
) -> Result<bool, MatchError>
where
    F: Fn(&str) -> Option<FuncSignature>,
{
    match &typ.node {
        TypKind::BoolT => Ok(matches!(value.kind, ValueKind::BoolV(_))),
        TypKind::NumT(num::Typ::NatT) => Ok(match &value.kind {
            ValueKind::NumV(num::T::Nat(_)) => true,
            ValueKind::NumV(num::T::Int(value)) => value.sign() != Sign::Minus,
            _ => false,
        }),
        TypKind::NumT(num::Typ::IntT) => Ok(matches!(value.kind, ValueKind::NumV(_))),
        TypKind::TextT => Ok(matches!(value.kind, ValueKind::TextV(_))),
        TypKind::VarT(type_id, type_args) => {
            let type_def =
                type_defs
                    .get(&type_id.node)
                    .ok_or_else(|| MatchError::UndefinedType {
                        name: type_id.node.clone(),
                        span: typ.span.clone(),
                    })?;
            match type_def {
                TypeDef::Param | TypeDef::Defining(_) => Err(MatchError::UnexpectedTypeVariable {
                    span: typ.span.clone(),
                }),
                TypeDef::Extern => Ok(matches!(value.kind, ValueKind::ExternV(_))),
                TypeDef::Defined(type_params, def_type) => {
                    if type_params.len() != type_args.len() {
                        return Err(MatchError::TypeArgumentMismatch {
                            expected: type_params.len(),
                            actual: type_args.len(),
                            span: typ.span.clone(),
                        });
                    }
                    let theta: TypeSubstitution = type_params
                        .iter()
                        .zip(type_args)
                        .map(|(type_param, type_arg)| (type_param.node.clone(), type_arg.clone()))
                        .collect();
                    match (&def_type.node, &value.kind) {
                        (DefTypKind::PlainT(typ), _) => {
                            let typ = subst::subst_type(&theta, typ)?;
                            sub_inner(type_defs, find_func, &typ, value)
                        }
                        (DefTypKind::StructT(type_fields), ValueKind::StructV(value_fields)) => {
                            if type_fields.len() != value_fields.len() {
                                return Ok(false);
                            }
                            for ((type_atom, typ), (value_atom, value)) in
                                type_fields.iter().zip(value_fields)
                            {
                                if type_atom.node != value_atom.node {
                                    return Ok(false);
                                }
                                let typ = subst::subst_type(&theta, typ)?;
                                if !sub_inner(type_defs, find_func, &typ, value)? {
                                    return Ok(false);
                                }
                            }
                            Ok(true)
                        }
                        (DefTypKind::VariantT(type_cases), ValueKind::CaseV(value_case)) => {
                            for (not_type, _, _) in type_cases {
                                if !not_type.node.same_shape(value_case) {
                                    continue;
                                }
                                let not_type = subst::subst_not_type(&theta, not_type)?;
                                let types = not_type.node.args();
                                let values = value_case.args();
                                if subs_inner(
                                    type_defs,
                                    find_func,
                                    types.into_iter(),
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
        TypKind::TupleT(types) => match &value.kind {
            ValueKind::TupleV(values) => subs_inner(
                type_defs,
                find_func,
                types.iter(),
                values.iter().map(AsRef::as_ref),
            ),
            _ => Ok(false),
        },
        TypKind::IterT(inner, Iter::Opt) => {
            if let ValueKind::OptV(Some(value)) = &value.kind {
                sub_inner(type_defs, find_func, inner, value)
            } else {
                Ok(true)
            }
        }
        TypKind::IterT(inner, Iter::List) => match &value.kind {
            ValueKind::ListV(values) => {
                for value in values {
                    if !sub_inner(type_defs, find_func, inner, value)? {
                        return Ok(false);
                    }
                }
                Ok(true)
            }
            _ => Ok(false),
        },
        TypKind::FuncT(type_params, param_types, return_type) => match &value.kind {
            ValueKind::FuncV(function_id) => {
                let signature =
                    find_func(&function_id.node).ok_or_else(|| MatchError::UndefinedFunction {
                        name: function_id.node.clone(),
                        span: function_id.span.clone(),
                    })?;
                Ok(equiv::equiv_func_type(
                    type_defs,
                    &typ.span,
                    type_params,
                    param_types,
                    return_type,
                    &signature.type_params,
                    &signature.param_types,
                    &signature.return_type,
                )?)
            }
            _ => Ok(false),
        },
    }
}

fn subs_inner<'typ, 'value, F, T, V>(
    type_defs: &TypeDefMap,
    find_func: &F,
    types: T,
    values: V,
) -> Result<bool, MatchError>
where
    F: Fn(&str) -> Option<FuncSignature>,
    T: ExactSizeIterator<Item = &'typ Typ>,
    V: ExactSizeIterator<Item = &'value Value>,
{
    if types.len() != values.len() {
        return Ok(false);
    }
    for (typ, value) in types.zip(values) {
        if !sub_inner(type_defs, find_func, typ, value)? {
            return Ok(false);
        }
    }
    Ok(true)
}

// Caches

// Caching subtyping of type variables to values,
// using semantic values because runtime values have no generated ids

pub type SubCache = HashMap<(String, ValueRef), bool>;

// Entry point

pub fn sub<F>(
    cache: &mut SubCache,
    type_defs: &TypeDefMap,
    find_func: &F,
    typ: &Typ,
    value: &ValueRef,
) -> Result<bool, MatchError>
where
    F: Fn(&str) -> Option<FuncSignature>,
{
    match &typ.node {
        TypKind::VarT(type_id, type_args) if type_args.is_empty() => {
            let key = (type_id.node.clone(), Rc::clone(value));
            if let Some(result) = cache.get(&key) {
                return Ok(*result);
            }
            let result = sub_inner(type_defs, find_func, typ, value)?;
            cache.insert(key, result);
            Ok(result)
        }
        _ => sub_inner(type_defs, find_func, typ, value),
    }
}

pub fn subs<F>(
    type_defs: &TypeDefMap,
    find_func: &F,
    types: &[Typ],
    values: &[ValueRef],
) -> Result<bool, MatchError>
where
    F: Fn(&str) -> Option<FuncSignature>,
{
    subs_inner(
        type_defs,
        find_func,
        types.iter(),
        values.iter().map(AsRef::as_ref),
    )
}
