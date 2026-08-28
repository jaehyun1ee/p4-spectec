use crate::lang::{
    il::ast::{self, DefTypKind, Iter, Subcheck, TypKind},
    xl::num,
};

use super::{
    FreshTypes, TypeDefinition, TypeEnvironment, TypeError, equivalent_notation_type,
    equivalent_type, expand_type, substitute_notation_type_with, substitution_from,
};

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
