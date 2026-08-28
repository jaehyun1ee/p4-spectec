use crate::lang::{
    common::{
        ds::map::IdMap,
        notation::mixfix::Mixfix,
        source::{Span, Spanned},
    },
    il::ast::{self, ParamKind, TypKind},
};

use super::{FreshTypes, TypeError, TypeErrorKind};

/// Type-variable replacements keyed by source-insensitive identifiers
pub type Substitution = IdMap<ast::Typ>;

pub(crate) fn substitution_from(
    parameters: &[ast::TParam],
    arguments: &[ast::Targ],
    span: &Span,
) -> Result<Substitution, TypeError> {
    if parameters.len() != arguments.len() {
        return Err(TypeError::new(
            TypeErrorKind::TypeArgumentCount {
                expected: parameters.len(),
                actual: arguments.len(),
            },
            span.clone(),
        ));
    }
    Ok(parameters
        .iter()
        .cloned()
        .zip(arguments.iter().cloned())
        .collect())
}

fn freshen_parameters(
    parameters: &[ast::TParam],
    fresh: &mut FreshTypes,
) -> (Substitution, Vec<ast::TParam>) {
    parameters
        .iter()
        .fold((Substitution::new(), Vec::new()), |mut state, parameter| {
            let (id, typ) = fresh.fresh();
            state.0.insert(parameter.clone(), typ);
            state.1.push(id);
            state
        })
}

pub(crate) fn substitute_type_with(
    substitution: &Substitution,
    typ: &ast::Typ,
    fresh: &mut FreshTypes,
) -> Result<ast::Typ, TypeError> {
    if substitution.is_empty() {
        return Ok(typ.clone());
    }
    match &typ.node {
        TypKind::Bool | TypKind::Num(_) | TypKind::Text => Ok(typ.clone()),
        TypKind::Var(id, arguments) => match substitution.get(id) {
            Some(_) if !arguments.is_empty() => Err(TypeError::new(
                TypeErrorKind::HigherOrderSubstitution,
                typ.span.clone(),
            )),
            Some(replacement) => Ok(replacement.clone()),
            None => Ok(Spanned::new(
                TypKind::Var(
                    id.clone(),
                    substitute_types_with(substitution, arguments, fresh)?,
                ),
                typ.span.clone(),
            )),
        },
        TypKind::Tuple(types) => Ok(Spanned::new(
            TypKind::Tuple(substitute_types_with(substitution, types, fresh)?),
            typ.span.clone(),
        )),
        TypKind::Iter(inner, iter) => {
            let inner = substitute_type_with(substitution, inner, fresh)?;
            let span = inner.span.clone();
            Ok(Spanned::new(TypKind::Iter(Box::new(inner), *iter), span))
        }
        TypKind::Func(parameters, parameter_types, result_type) => {
            let (freshening, parameters) = freshen_parameters(parameters, fresh);
            let parameter_types = substitute_types_with(&freshening, parameter_types, fresh)?;
            let parameter_types = substitute_types_with(substitution, &parameter_types, fresh)?;
            let result_type = substitute_type_with(&freshening, result_type, fresh)?;
            let result_type = substitute_type_with(substitution, &result_type, fresh)?;
            Ok(Spanned::new(
                TypKind::Func(parameters, parameter_types, Box::new(result_type)),
                typ.span.clone(),
            ))
        }
    }
}

pub(crate) fn substitute_types_with(
    substitution: &Substitution,
    types: &[ast::Typ],
    fresh: &mut FreshTypes,
) -> Result<Vec<ast::Typ>, TypeError> {
    types
        .iter()
        .map(|typ| substitute_type_with(substitution, typ, fresh))
        .collect()
}

/// Substitutes type variables while freshening nested function binders
pub fn substitute_type(substitution: &Substitution, typ: &ast::Typ) -> Result<ast::Typ, TypeError> {
    substitute_type_with(substitution, typ, &mut FreshTypes)
}

/// Substitutes type variables in a type list
pub fn substitute_types(
    substitution: &Substitution,
    types: &[ast::Typ],
) -> Result<Vec<ast::Typ>, TypeError> {
    substitute_types_with(substitution, types, &mut FreshTypes)
}

fn substitute_notation_node(
    substitution: &Substitution,
    notation: &Mixfix<ast::Typ>,
    fresh: &mut FreshTypes,
) -> Result<Mixfix<ast::Typ>, TypeError> {
    match notation {
        Mixfix::Arg(typ) => Ok(Mixfix::Arg(substitute_type_with(substitution, typ, fresh)?)),
        Mixfix::Atom(atom) => Ok(Mixfix::Atom(atom.clone())),
        Mixfix::Brack(atom_l, notation, atom_r) => Ok(Mixfix::Brack(
            atom_l.clone(),
            Box::new(substitute_notation_node(substitution, notation, fresh)?),
            atom_r.clone(),
        )),
        Mixfix::Infix(notation_l, atom, notation_r) => Ok(Mixfix::Infix(
            Box::new(substitute_notation_node(substitution, notation_l, fresh)?),
            atom.clone(),
            Box::new(substitute_notation_node(substitution, notation_r, fresh)?),
        )),
        Mixfix::Seq(notations) => Ok(Mixfix::Seq(
            notations
                .iter()
                .map(|notation| substitute_notation_node(substitution, notation, fresh))
                .collect::<Result<_, _>>()?,
        )),
    }
}

pub(crate) fn substitute_notation_type_with(
    substitution: &Substitution,
    notation_type: &ast::NotTyp,
    fresh: &mut FreshTypes,
) -> Result<ast::NotTyp, TypeError> {
    if substitution.is_empty() {
        return Ok(notation_type.clone());
    }
    Ok(Spanned::new(
        substitute_notation_node(substitution, &notation_type.node, fresh)?,
        notation_type.span.clone(),
    ))
}

/// Substitutes type variables in a notation type
pub fn substitute_notation_type(
    substitution: &Substitution,
    notation_type: &ast::NotTyp,
) -> Result<ast::NotTyp, TypeError> {
    substitute_notation_type_with(substitution, notation_type, &mut FreshTypes)
}

/// Substitutes type variables in a variant case
pub fn substitute_type_case(
    substitution: &Substitution,
    type_case: &ast::TypCase,
) -> Result<ast::TypCase, TypeError> {
    let mut fresh = FreshTypes;
    let (notation_type, origin, hints) = type_case;
    let (id, arguments) = &origin.node;
    Ok((
        substitute_notation_type_with(substitution, notation_type, &mut fresh)?,
        Spanned::new(
            (
                id.clone(),
                substitute_types_with(substitution, arguments, &mut fresh)?,
            ),
            origin.span.clone(),
        ),
        hints.clone(),
    ))
}

fn substitute_parameter_with(
    substitution: &Substitution,
    parameter: &ast::Param,
    fresh: &mut FreshTypes,
) -> Result<ast::Param, TypeError> {
    let node = match &parameter.node {
        ParamKind::Exp(typ) => ParamKind::Exp(substitute_type_with(substitution, typ, fresh)?),
        ParamKind::Def(id, parameters, nested, typ) => {
            let (freshening, parameters) = freshen_parameters(parameters, fresh);
            let nested = substitute_parameters_with(&freshening, nested, fresh)?;
            let nested = substitute_parameters_with(substitution, &nested, fresh)?;
            let typ = substitute_type_with(&freshening, typ, fresh)?;
            let typ = substitute_type_with(substitution, &typ, fresh)?;
            ParamKind::Def(id.clone(), parameters, nested, typ)
        }
    };
    Ok(Spanned::new(node, parameter.span.clone()))
}

fn substitute_parameters_with(
    substitution: &Substitution,
    parameters: &[ast::Param],
    fresh: &mut FreshTypes,
) -> Result<Vec<ast::Param>, TypeError> {
    parameters
        .iter()
        .map(|parameter| substitute_parameter_with(substitution, parameter, fresh))
        .collect()
}

/// Substitutes type variables in a callable parameter
pub fn substitute_parameter(
    substitution: &Substitution,
    parameter: &ast::Param,
) -> Result<ast::Param, TypeError> {
    substitute_parameter_with(substitution, parameter, &mut FreshTypes)
}

/// Substitutes type variables in callable parameters
pub fn substitute_parameters(
    substitution: &Substitution,
    parameters: &[ast::Param],
) -> Result<Vec<ast::Param>, TypeError> {
    substitute_parameters_with(substitution, parameters, &mut FreshTypes)
}
