use thiserror::Error;

use crate::{
    domain::source::Region,
    lang::{
        il::ast::{DefTypKind, Iter, NotTyp, TParam, Typ, TypKind},
        xl::num,
    },
};

use super::{
    envs::TypeDefMap,
    equiv::{self, EquivError},
    expand::{self, ExpandError},
    subst::{self, SubstError, TypeSubstitution},
    typdef::TypeDef,
};

#[derive(Clone, Debug, Error, PartialEq)]
pub enum SubError {
    #[error("type arguments do not match at {span}")]
    TypeArgumentMismatch { span: Region },
    #[error(transparent)]
    Equivalence(#[from] EquivError),
    #[error(transparent)]
    Expansion(#[from] ExpandError),
    #[error(transparent)]
    Substitution(#[from] SubstError),
}

pub fn sub_type(type_defs: &TypeDefMap, typ_a: &Typ, typ_b: &Typ) -> Result<bool, SubError> {
    if equiv::equiv_type(type_defs, typ_a, typ_b)? {
        Ok(true)
    } else {
        sub_type_inner(type_defs, typ_a, typ_b)
    }
}

fn sub_type_inner(type_defs: &TypeDefMap, typ_a: &Typ, typ_b: &Typ) -> Result<bool, SubError> {
    let typ_a = expand::expand_type(type_defs, typ_a)?;
    let typ_b = expand::expand_type(type_defs, typ_b)?;
    match (&typ_a.node, &typ_b.node) {
        (TypKind::NumT(num_type_a), TypKind::NumT(num_type_b)) => {
            Ok(num::sub(*num_type_a, *num_type_b))
        }
        (TypKind::VarT(type_id_a, type_args_a), TypKind::VarT(type_id_b, type_args_b)) => match (
            type_defs.get(&type_id_a.node),
            type_defs.get(&type_id_b.node),
        ) {
            (
                Some(TypeDef::Defined(type_params_a, def_type_a)),
                Some(TypeDef::Defined(type_params_b, def_type_b)),
            ) => match (&def_type_a.node, &def_type_b.node) {
                (DefTypKind::VariantT(type_cases_a), DefTypKind::VariantT(type_cases_b)) => {
                    let theta_a = type_substitution(type_params_a, type_args_a, &typ_a.span)?;
                    let theta_b = type_substitution(type_params_b, type_args_b, &typ_b.span)?;
                    let not_types_a = subst_not_types(&theta_a, type_cases_a)?;
                    let not_types_b = subst_not_types(&theta_b, type_cases_b)?;
                    not_types_subset(type_defs, &not_types_a, &not_types_b)
                }
                _ => Ok(false),
            },
            _ => Ok(false),
        },
        (TypKind::TupleT(types_a), TypKind::TupleT(types_b)) => {
            if types_a.len() != types_b.len() {
                return Ok(false);
            }
            for (typ_a, typ_b) in types_a.iter().zip(types_b) {
                if !sub_type(type_defs, typ_a, typ_b)? {
                    return Ok(false);
                }
            }
            Ok(true)
        }
        (TypKind::IterT(typ_a, iter_a), TypKind::IterT(typ_b, iter_b)) if iter_a == iter_b => {
            sub_type(type_defs, typ_a, typ_b)
        }
        (TypKind::IterT(typ_a, Iter::Opt), TypKind::IterT(typ_b, Iter::List)) => {
            sub_type(type_defs, typ_a, typ_b)
        }
        (_, TypKind::IterT(typ_b, Iter::Opt)) => sub_type(type_defs, &typ_a, typ_b),
        (_, TypKind::IterT(typ_b, Iter::List)) => sub_type(type_defs, &typ_a, typ_b),
        _ => Ok(false),
    }
}

fn type_substitution(
    type_params: &[TParam],
    type_args: &[Typ],
    span: &Region,
) -> Result<TypeSubstitution, SubError> {
    if type_params.len() != type_args.len() {
        return Err(SubError::TypeArgumentMismatch { span: span.clone() });
    }
    Ok(type_params
        .iter()
        .zip(type_args)
        .map(|(type_param, type_arg)| (type_param.node.clone(), type_arg.clone()))
        .collect())
}

fn subst_not_types(
    theta: &TypeSubstitution,
    type_cases: &[crate::lang::il::ast::TypCase],
) -> Result<Vec<NotTyp>, SubError> {
    type_cases
        .iter()
        .map(|(not_type, _, _)| Ok(subst::subst_not_type(theta, not_type)?))
        .collect()
}

fn not_types_subset(
    type_defs: &TypeDefMap,
    not_types_a: &[NotTyp],
    not_types_b: &[NotTyp],
) -> Result<bool, SubError> {
    for not_type_a in not_types_a {
        let mut found = false;
        for not_type_b in not_types_b {
            if equiv::equiv_not_type(type_defs, not_type_a, not_type_b)? {
                found = true;
                break;
            }
        }
        if !found {
            return Ok(false);
        }
    }
    Ok(true)
}
