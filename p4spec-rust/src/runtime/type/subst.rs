use std::collections::BTreeMap;

use thiserror::Error;

use crate::{
    domain::{
        mixfix::Mixfix,
        source::{Region, Spanned},
    },
    lang::il::ast::{self as il, ParamKind, TParam, Typ, TypCase, TypKind, TypOrigin},
};

use super::fresh;

// Substitution of type variables

#[derive(Clone, Debug, Default, PartialEq)]
pub struct TypeSubstitution(BTreeMap<String, Typ>);

impl TypeSubstitution {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn insert(&mut self, id: il::Id, typ: Typ) -> Option<Typ> {
        self.0.insert(id.node, typ)
    }

    pub fn get(&self, id: &il::Id) -> Option<&Typ> {
        self.0.get(&id.node)
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}

#[derive(Clone, Debug, Error, PartialEq)]
pub enum SubstError {
    #[error("higher-order substitution is disallowed at {span}")]
    HigherOrder { span: Region },
}

pub fn freshen_tparams(type_params: &[TParam]) -> (TypeSubstitution, Vec<TParam>) {
    type_params.iter().fold(
        (TypeSubstitution::new(), Vec::new()),
        |(mut theta, mut fresh_params), type_param| {
            let fresh_id = Spanned::new(format!("__FRESH{}", fresh::fresh()), Region::none());
            let fresh_type =
                Spanned::new(TypKind::VarT(fresh_id.clone(), Vec::new()), Region::none());
            theta.insert(type_param.clone(), fresh_type);
            fresh_params.push(fresh_id);
            (theta, fresh_params)
        },
    )
}

// Types

fn subst_type_inner(theta: &TypeSubstitution, typ: &Typ) -> Result<Typ, SubstError> {
    match &typ.node {
        TypKind::BoolT | TypKind::NumT(_) | TypKind::TextT => Ok(typ.clone()),
        TypKind::VarT(id, type_args) => match theta.get(id) {
            Some(_) if !type_args.is_empty() => Err(SubstError::HigherOrder {
                span: typ.span.clone(),
            }),
            Some(typ) => Ok(typ.clone()),
            None => Ok(Spanned::new(
                TypKind::VarT(id.clone(), subst_types_inner(theta, type_args)?),
                typ.span.clone(),
            )),
        },
        TypKind::TupleT(types) => Ok(Spanned::new(
            TypKind::TupleT(subst_types_inner(theta, types)?),
            typ.span.clone(),
        )),
        TypKind::IterT(inner, iter) => {
            let inner = subst_type_inner(theta, inner)?;
            let span = inner.span.clone();
            Ok(Spanned::new(TypKind::IterT(Box::new(inner), *iter), span))
        }
        TypKind::FuncT(type_params, param_types, return_type) => {
            let (fresh_theta, fresh_params) = freshen_tparams(type_params);
            let param_types = subst_types_inner(&fresh_theta, param_types)?;
            let param_types = subst_types_inner(theta, &param_types)?;
            let return_type = subst_type_inner(&fresh_theta, return_type)?;
            let return_type = subst_type_inner(theta, &return_type)?;
            Ok(Spanned::new(
                TypKind::FuncT(fresh_params, param_types, Box::new(return_type)),
                typ.span.clone(),
            ))
        }
    }
}

fn subst_types_inner(theta: &TypeSubstitution, types: &[Typ]) -> Result<Vec<Typ>, SubstError> {
    types
        .iter()
        .map(|typ| subst_type_inner(theta, typ))
        .collect()
}

pub fn subst_type(theta: &TypeSubstitution, typ: &Typ) -> Result<Typ, SubstError> {
    if theta.is_empty() {
        Ok(typ.clone())
    } else {
        subst_type_inner(theta, typ)
    }
}

pub fn subst_types(theta: &TypeSubstitution, types: &[Typ]) -> Result<Vec<Typ>, SubstError> {
    if theta.is_empty() {
        Ok(types.to_vec())
    } else {
        subst_types_inner(theta, types)
    }
}

// Variant types

fn subst_notation(
    theta: &TypeSubstitution,
    notation: &Mixfix<Typ>,
) -> Result<Mixfix<Typ>, SubstError> {
    match notation {
        Mixfix::Arg(typ) => Ok(Mixfix::Arg(subst_type(theta, typ)?)),
        Mixfix::Atom(atom) => Ok(Mixfix::Atom(atom.clone())),
        Mixfix::Brack(left, body, right) => Ok(Mixfix::Brack(
            left.clone(),
            Box::new(subst_notation(theta, body)?),
            right.clone(),
        )),
        Mixfix::Infix(left, atom, right) => Ok(Mixfix::Infix(
            Box::new(subst_notation(theta, left)?),
            atom.clone(),
            Box::new(subst_notation(theta, right)?),
        )),
        Mixfix::Seq(items) => Ok(Mixfix::Seq(
            items
                .iter()
                .map(|item| subst_notation(theta, item))
                .collect::<Result<_, _>>()?,
        )),
    }
}

pub fn subst_not_type(
    theta: &TypeSubstitution,
    not_type: &il::NotTyp,
) -> Result<il::NotTyp, SubstError> {
    if theta.is_empty() {
        Ok(not_type.clone())
    } else {
        Ok(Spanned::new(
            subst_notation(theta, &not_type.node)?,
            not_type.span.clone(),
        ))
    }
}

pub fn subst_type_case(
    theta: &TypeSubstitution,
    type_case: &TypCase,
) -> Result<TypCase, SubstError> {
    let (not_type, origin, hints) = type_case;
    let not_type = subst_not_type(theta, not_type)?;
    let (id, type_args) = &origin.node;
    let origin = TypOrigin::new(
        (id.clone(), subst_types(theta, type_args)?),
        origin.span.clone(),
    );
    Ok((not_type, origin, hints.clone()))
}

// Parameters

pub fn subst_param(theta: &TypeSubstitution, param: &il::Param) -> Result<il::Param, SubstError> {
    match &param.node {
        ParamKind::ExpP(typ) => Ok(Spanned::new(
            ParamKind::ExpP(subst_type(theta, typ)?),
            param.span.clone(),
        )),
        ParamKind::DefP(id, type_params, params, return_type) => {
            let (fresh_theta, fresh_params) = freshen_tparams(type_params);
            let params = subst_params(&fresh_theta, params)?;
            let params = subst_params(theta, &params)?;
            let return_type = subst_type(&fresh_theta, return_type)?;
            let return_type = subst_type(theta, &return_type)?;
            Ok(Spanned::new(
                ParamKind::DefP(id.clone(), fresh_params, params, return_type),
                param.span.clone(),
            ))
        }
    }
}

pub fn subst_params(
    theta: &TypeSubstitution,
    params: &[il::Param],
) -> Result<Vec<il::Param>, SubstError> {
    params
        .iter()
        .map(|param| subst_param(theta, param))
        .collect()
}
