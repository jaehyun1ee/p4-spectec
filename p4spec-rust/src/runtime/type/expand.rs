use thiserror::Error;

use crate::{
    domain::source::Region,
    lang::il::ast::{DefTypKind, Typ, TypKind},
};

use super::{
    envs::TypeDefMap,
    subst::{self, SubstError, TypeSubstitution},
    typdef::TypeDef,
};

// Type expansion

#[derive(Clone, Debug, Error, PartialEq)]
pub enum ExpandError {
    #[error("type arguments do not match at {span}")]
    TypeArgumentMismatch { span: Region },
    #[error("type variable {name} is not defined at {span}")]
    UndefinedType { name: String, span: Region },
    #[error(transparent)]
    Substitution(#[from] SubstError),
}

pub(crate) fn expand_type_with<'a>(
    find_type_def: &impl Fn(&str) -> Option<&'a TypeDef>,
    typ: &Typ,
) -> Result<Typ, ExpandError> {
    match &typ.node {
        TypKind::VarT(type_id, type_args) => match find_type_def(&type_id.node) {
            Some(TypeDef::Defined(type_params, def_type)) => match &def_type.node {
                DefTypKind::PlainT(_) if type_args.len() != type_params.len() => {
                    Err(ExpandError::TypeArgumentMismatch {
                        span: typ.span.clone(),
                    })
                }
                DefTypKind::PlainT(typ) => {
                    let theta: TypeSubstitution = type_params
                        .iter()
                        .zip(type_args)
                        .map(|(type_param, type_arg)| (type_param.node.clone(), type_arg.clone()))
                        .collect();
                    let typ = subst::subst_type(&theta, typ)?;
                    expand_type_with(find_type_def, &typ)
                }
                _ => Ok(typ.clone()),
            },
            Some(_) => Ok(typ.clone()),
            None => Err(ExpandError::UndefinedType {
                name: type_id.node.clone(),
                span: typ.span.clone(),
            }),
        },
        _ => Ok(typ.clone()),
    }
}

pub fn expand_type(type_defs: &TypeDefMap, typ: &Typ) -> Result<Typ, ExpandError> {
    expand_type_with(&|type_id| type_defs.get(type_id), typ)
}
