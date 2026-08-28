use crate::lang::{
    common::{notation::mixfix::Mixfix, source::Span},
    il::ast::{self, TypKind},
    xl::num,
};

use super::{
    FreshTypes, Substitution, TypeDefinition, TypeEnvironment, TypeError, TypeErrorKind,
    expand_type, substitute_type_with, substitute_types_with,
};

/// Tests semantic type equivalence after expanding plain aliases
pub fn equivalent_type(
    environment: &TypeEnvironment,
    typ_l: &ast::Typ,
    typ_r: &ast::Typ,
) -> Result<bool, TypeError> {
    let typ_l = expand_type(environment, typ_l)?;
    let typ_r = expand_type(environment, typ_r)?;
    match (&typ_l.node, &typ_r.node) {
        (TypKind::Bool, TypKind::Bool) | (TypKind::Text, TypKind::Text) => Ok(true),
        (TypKind::Num(number_l), TypKind::Num(number_r)) => Ok(num::equiv(*number_l, *number_r)),
        (TypKind::Var(id_l, args_l), TypKind::Var(id_r, args_r)) => {
            if id_l.node != id_r.node || args_l.len() != args_r.len() {
                return Ok(false);
            }
            for (arg_l, arg_r) in args_l.iter().zip(args_r) {
                if !equivalent_type(environment, arg_l, arg_r)? {
                    return Ok(false);
                }
            }
            Ok(true)
        }
        (TypKind::Tuple(types_l), TypKind::Tuple(types_r)) => {
            if types_l.len() != types_r.len() {
                return Ok(false);
            }
            for (typ_l, typ_r) in types_l.iter().zip(types_r) {
                if !equivalent_type(environment, typ_l, typ_r)? {
                    return Ok(false);
                }
            }
            Ok(true)
        }
        (TypKind::Iter(typ_l, iter_l), TypKind::Iter(typ_r, iter_r)) => {
            Ok(iter_l == iter_r && equivalent_type(environment, typ_l, typ_r)?)
        }
        _ => Ok(false),
    }
}

fn equivalent_notation_node(
    environment: &TypeEnvironment,
    notation_l: &Mixfix<ast::Typ>,
    notation_r: &Mixfix<ast::Typ>,
) -> Result<bool, TypeError> {
    match (notation_l, notation_r) {
        (Mixfix::Arg(typ_l), Mixfix::Arg(typ_r)) => equivalent_type(environment, typ_l, typ_r),
        (Mixfix::Atom(atom_l), Mixfix::Atom(atom_r)) => Ok(atom_l.node == atom_r.node),
        (
            Mixfix::Brack(atom_l_l, notation_l, atom_l_r),
            Mixfix::Brack(atom_r_l, notation_r, atom_r_r),
        ) => Ok(atom_l_l.node == atom_r_l.node
            && atom_l_r.node == atom_r_r.node
            && equivalent_notation_node(environment, notation_l, notation_r)?),
        (
            Mixfix::Infix(notation_l_l, atom_l, notation_l_r),
            Mixfix::Infix(notation_r_l, atom_r, notation_r_r),
        ) => Ok(atom_l.node == atom_r.node
            && equivalent_notation_node(environment, notation_l_l, notation_r_l)?
            && equivalent_notation_node(environment, notation_l_r, notation_r_r)?),
        (Mixfix::Seq(notations_l), Mixfix::Seq(notations_r)) => {
            if notations_l.len() != notations_r.len() {
                return Ok(false);
            }
            for (notation_l, notation_r) in notations_l.iter().zip(notations_r) {
                if !equivalent_notation_node(environment, notation_l, notation_r)? {
                    return Ok(false);
                }
            }
            Ok(true)
        }
        _ => Ok(false),
    }
}

/// Tests semantic equivalence of notation types
pub fn equivalent_notation_type(
    environment: &TypeEnvironment,
    notation_l: &ast::NotTyp,
    notation_r: &ast::NotTyp,
) -> Result<bool, TypeError> {
    equivalent_notation_node(environment, &notation_l.node, &notation_r.node)
}

/// Tests alpha-equivalence of two callable type signatures
#[allow(clippy::too_many_arguments)]
pub fn equivalent_function_type(
    environment: &TypeEnvironment,
    span: &Span,
    type_parameters_l: &[ast::TParam],
    parameter_types_l: &[ast::Typ],
    result_type_l: &ast::Typ,
    type_parameters_r: &[ast::TParam],
    parameter_types_r: &[ast::Typ],
    result_type_r: &ast::Typ,
) -> Result<bool, TypeError> {
    if type_parameters_l.len() != type_parameters_r.len() {
        return Err(TypeError::new(
            TypeErrorKind::TypeParameterCount {
                left: type_parameters_l.len(),
                right: type_parameters_r.len(),
            },
            span.clone(),
        ));
    }
    if parameter_types_l.len() != parameter_types_r.len() {
        return Err(TypeError::new(
            TypeErrorKind::ParameterCount {
                left: parameter_types_l.len(),
                right: parameter_types_r.len(),
            },
            span.clone(),
        ));
    }

    let mut fresh = FreshTypes;
    let mut substitution_l = Substitution::new();
    let mut substitution_r = Substitution::new();
    let mut environment = environment.clone();
    for (parameter_l, parameter_r) in type_parameters_l.iter().zip(type_parameters_r) {
        let (id, typ) = fresh.fresh();
        environment.insert(id, TypeDefinition::Parameter);
        substitution_l.insert(parameter_l.clone(), typ.clone());
        substitution_r.insert(parameter_r.clone(), typ);
    }

    let parameter_types_l = substitute_types_with(&substitution_l, parameter_types_l, &mut fresh)?;
    let parameter_types_r = substitute_types_with(&substitution_r, parameter_types_r, &mut fresh)?;
    let result_type_l = substitute_type_with(&substitution_l, result_type_l, &mut fresh)?;
    let result_type_r = substitute_type_with(&substitution_r, result_type_r, &mut fresh)?;

    for (typ_l, typ_r) in parameter_types_l.iter().zip(&parameter_types_r) {
        if !equivalent_type(&environment, typ_l, typ_r)? {
            return Ok(false);
        }
    }
    equivalent_type(&environment, &result_type_l, &result_type_r)
}
