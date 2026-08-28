use crate::lang::il::ast::{self, DefTypKind, TypKind};

use super::{TypeDefinition, TypeEnvironment, TypeError, TypeErrorKind, substitution_from};

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
