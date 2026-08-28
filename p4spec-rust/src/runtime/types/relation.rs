use crate::lang::{
    common::{notation::mixfix::Mixfix, source::Span},
    il::ast::{self, DefTypKind, Iter, Subcheck, TypKind},
    xl::num,
};

use super::{
    FreshTypes, Substitution, TypeDefinition, TypeEnvironment, TypeError, TypeErrorKind,
    substitute_notation_type_with, substitute_type_with, substitute_types_with, substitution_from,
};

/// Expands an outermost plain type alias until a non-alias type is reached
pub fn expand_type(environment: &TypeEnvironment, typ: &ast::Typ) -> Result<ast::Typ, TypeError> {
    let TypKind::Var(id, arguments) = &typ.node else {
        return Ok(typ.clone());
    };
    match environment.get(id) {
        Some(TypeDefinition::Defined(parameters, def_type)) => match &def_type.node {
            DefTypKind::Plain(alias) => {
                let substitution = substitution_from(parameters, arguments, &typ.span)?;
                let expanded = super::substitute_type(&substitution, alias)?;
                expand_type(environment, &expanded)
            }
            DefTypKind::Struct(_) | DefTypKind::Variant(_) => Ok(typ.clone()),
        },
        Some(TypeDefinition::Parameter | TypeDefinition::Extern | TypeDefinition::Defining(_)) => {
            Ok(typ.clone())
        }
        None => Err(TypeError::new(
            TypeErrorKind::UndefinedType(id.node.clone()),
            typ.span.clone(),
        )),
    }
}

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

fn substituted_variant_cases(
    environment: &TypeEnvironment,
    typ: &ast::Typ,
) -> Result<Option<Vec<ast::NotTyp>>, TypeError> {
    let TypKind::Var(id, arguments) = &typ.node else {
        return Ok(None);
    };
    let Some(TypeDefinition::Defined(parameters, def_type)) = environment.get(id) else {
        return Ok(None);
    };
    let DefTypKind::Variant(cases) = &def_type.node else {
        return Ok(None);
    };
    let substitution = substitution_from(parameters, arguments, &typ.span)?;
    let mut fresh = FreshTypes;
    cases
        .iter()
        .map(|(notation_type, _, _)| {
            substitute_notation_type_with(&substitution, notation_type, &mut fresh)
        })
        .collect::<Result<Vec<_>, _>>()
        .map(Some)
}

/// Tests whether the source type is a subtype of the target type
pub fn is_subtype(
    environment: &TypeEnvironment,
    source: &ast::Typ,
    target: &ast::Typ,
) -> Result<bool, TypeError> {
    if equivalent_type(environment, source, target)? {
        return Ok(true);
    }
    let source = expand_type(environment, source)?;
    let target = expand_type(environment, target)?;
    match (&source.node, &target.node) {
        (TypKind::Num(source), TypKind::Num(target)) => Ok(num::sub(*source, *target)),
        (TypKind::Var(_, _), TypKind::Var(_, _)) => {
            let Some(source_cases) = substituted_variant_cases(environment, &source)? else {
                return Ok(false);
            };
            let Some(target_cases) = substituted_variant_cases(environment, &target)? else {
                return Ok(false);
            };
            for source_case in source_cases {
                let mut found = false;
                for target_case in &target_cases {
                    if equivalent_notation_type(environment, &source_case, target_case)? {
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
        (TypKind::Tuple(source), TypKind::Tuple(target)) => {
            if source.len() != target.len() {
                return Ok(false);
            }
            for (source, target) in source.iter().zip(target) {
                if !is_subtype(environment, source, target)? {
                    return Ok(false);
                }
            }
            Ok(true)
        }
        (TypKind::Iter(source, source_iter), TypKind::Iter(target, target_iter))
            if source_iter == target_iter
                || (*source_iter == Iter::Opt && *target_iter == Iter::List) =>
        {
            is_subtype(environment, source, target)
        }
        (_, TypKind::Iter(target, Iter::Opt | Iter::List)) => {
            is_subtype(environment, &source, target)
        }
        _ => Ok(false),
    }
}

/// Builds the least runtime subtype check needed after static subtyping
pub fn optimize_subtype(
    environment: &TypeEnvironment,
    source: &ast::Typ,
    target: &ast::Typ,
) -> Result<Subcheck, TypeError> {
    if is_subtype(environment, source, target)? {
        return Ok(Subcheck::Skip);
    }
    let source_expanded = expand_type(environment, source)?;
    let target_expanded = expand_type(environment, target)?;
    match (&source_expanded.node, &target_expanded.node) {
        (TypKind::Tuple(source), TypKind::Tuple(target)) if source.len() == target.len() => {
            Ok(Subcheck::Tuple(
                source
                    .iter()
                    .zip(target)
                    .map(|(source, target)| optimize_subtype(environment, source, target))
                    .collect::<Result<_, _>>()?,
            ))
        }
        (TypKind::Iter(source, source_iter), TypKind::Iter(target, target_iter))
            if source_iter == target_iter =>
        {
            Ok(Subcheck::Iter(
                *source_iter,
                Box::new(optimize_subtype(environment, source, target)?),
            ))
        }
        (TypKind::Var(source_id, _), TypKind::Var(target_id, _))
            if is_subtype(environment, &target_expanded, &source_expanded)? =>
        {
            match (environment.get(source_id), environment.get(target_id)) {
                (
                    Some(TypeDefinition::Defined(_, source_definition)),
                    Some(TypeDefinition::Defined(_, target_definition)),
                ) => match (&source_definition.node, &target_definition.node) {
                    (DefTypKind::Variant(_), DefTypKind::Variant(target_cases)) => {
                        Ok(Subcheck::Mixop(
                            target_cases
                                .iter()
                                .map(|(notation_type, _, _)| notation_type.node.to_mixop())
                                .collect(),
                        ))
                    }
                    _ => Ok(Subcheck::Recurse(target.clone())),
                },
                _ => Ok(Subcheck::Recurse(target.clone())),
            }
        }
        _ => Ok(Subcheck::Recurse(target.clone())),
    }
}
