use thiserror::Error;

use crate::{
    domain::source::{Region, Spanned},
    lang::{
        il::ast::{NotTyp, TParam, Typ, TypKind},
        xl::num,
    },
};

use super::{
    envs::TypeDefMap,
    expand::{self, ExpandError},
    fresh,
    subst::{self, SubstError, TypeSubstitution},
    typdef::TypeDef,
};

// Type equivalence and subtyping

#[derive(Clone, Debug, Error, PartialEq)]
pub enum EquivError {
    #[error("type parameters do not match at {span}")]
    TypeParametersMismatch { span: Region },
    #[error("parameters do not match at {span}")]
    ParametersMismatch { span: Region },
    #[error(transparent)]
    Expansion(#[from] ExpandError),
    #[error(transparent)]
    Substitution(#[from] SubstError),
}

fn equiv_types_with<'a>(
    find_type_def: &impl Fn(&str) -> Option<&'a TypeDef>,
    types_a: &[Typ],
    types_b: &[Typ],
) -> Result<bool, EquivError> {
    if types_a.len() != types_b.len() {
        return Ok(false);
    }
    for (typ_a, typ_b) in types_a.iter().zip(types_b) {
        if !equiv_type_with(find_type_def, typ_a, typ_b)? {
            return Ok(false);
        }
    }
    Ok(true)
}

fn equiv_type_with<'a>(
    find_type_def: &impl Fn(&str) -> Option<&'a TypeDef>,
    typ_a: &Typ,
    typ_b: &Typ,
) -> Result<bool, EquivError> {
    let typ_a = expand::expand_type_with(find_type_def, typ_a)?;
    let typ_b = expand::expand_type_with(find_type_def, typ_b)?;
    match (&typ_a.node, &typ_b.node) {
        (TypKind::BoolT, TypKind::BoolT) => Ok(true),
        (TypKind::NumT(num_type_a), TypKind::NumT(num_type_b)) => {
            Ok(num::equiv(*num_type_a, *num_type_b))
        }
        (TypKind::TextT, TypKind::TextT) => Ok(true),
        (TypKind::VarT(type_id_a, type_args_a), TypKind::VarT(type_id_b, type_args_b)) => {
            if type_id_a.node != type_id_b.node || type_args_a.len() != type_args_b.len() {
                return Ok(false);
            }
            equiv_types_with(find_type_def, type_args_a, type_args_b)
        }
        (TypKind::TupleT(types_a), TypKind::TupleT(types_b)) => {
            equiv_types_with(find_type_def, types_a, types_b)
        }
        (TypKind::IterT(typ_a, iter_a), TypKind::IterT(typ_b, iter_b)) => {
            Ok(equiv_type_with(find_type_def, typ_a, typ_b)? && iter_a == iter_b)
        }
        _ => Ok(false),
    }
}

pub fn equiv_type(type_defs: &TypeDefMap, typ_a: &Typ, typ_b: &Typ) -> Result<bool, EquivError> {
    equiv_type_with(&|type_id| type_defs.get(type_id), typ_a, typ_b)
}

fn equiv_not_type_with<'a>(
    find_type_def: &impl Fn(&str) -> Option<&'a TypeDef>,
    not_type_a: &NotTyp,
    not_type_b: &NotTyp,
) -> Result<bool, EquivError> {
    not_type_a.node.try_eq_by(&not_type_b.node, |typ_a, typ_b| {
        equiv_type_with(find_type_def, typ_a, typ_b)
    })
}

pub fn equiv_not_type(
    type_defs: &TypeDefMap,
    not_type_a: &NotTyp,
    not_type_b: &NotTyp,
) -> Result<bool, EquivError> {
    equiv_not_type_with(&|type_id| type_defs.get(type_id), not_type_a, not_type_b)
}

#[allow(clippy::too_many_arguments)]
pub fn equiv_func_type(
    type_defs: &TypeDefMap,
    span: &Region,
    type_params_a: &[TParam],
    param_types_a: &[Typ],
    typ_a: &Typ,
    type_params_b: &[TParam],
    param_types_b: &[Typ],
    typ_b: &Typ,
) -> Result<bool, EquivError> {
    if type_params_a.len() != type_params_b.len() {
        return Err(EquivError::TypeParametersMismatch { span: span.clone() });
    }
    let mut fresh_type_defs = TypeDefMap::with_capacity(type_params_a.len());
    let mut theta_a = TypeSubstitution::with_capacity(type_params_a.len());
    let mut theta_b = TypeSubstitution::with_capacity(type_params_b.len());
    for (type_param_a, type_param_b) in type_params_a.iter().zip(type_params_b) {
        let fresh_type_id = format!("__FRESH{}", fresh::fresh());
        let fresh_type = Spanned::new(
            TypKind::VarT(
                Spanned::new(fresh_type_id.clone(), Region::none()),
                Vec::new(),
            ),
            Region::none(),
        );
        fresh_type_defs.insert(fresh_type_id, TypeDef::Param);
        theta_a.insert(type_param_a.node.clone(), fresh_type.clone());
        theta_b.insert(type_param_b.node.clone(), fresh_type);
    }
    let find_type_def = |type_id: &str| {
        fresh_type_defs
            .get(type_id)
            .or_else(|| type_defs.get(type_id))
    };
    if param_types_a.len() != param_types_b.len() {
        return Err(EquivError::ParametersMismatch { span: span.clone() });
    }
    let param_types_a = subst::subst_types(&theta_a, param_types_a)?;
    let param_types_b = subst::subst_types(&theta_b, param_types_b)?;
    let typ_a = subst::subst_type(&theta_a, typ_a)?;
    let typ_b = subst::subst_type(&theta_b, typ_b)?;
    Ok(
        equiv_types_with(&find_type_def, &param_types_a, &param_types_b)?
            && equiv_type_with(&find_type_def, &typ_a, &typ_b)?,
    )
}
