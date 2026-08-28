//! Expansion of intermediate-language type aliases

use crate::lang::il::ast::{self, DefTypKind, TypKind};

use super::{TDEnv, Theta, TypeArityMismatch, TypeDef, TypeError, TypeErrorKind, subst_typ};

/// Expands plain type aliases until a non-alias type is reached
pub fn expand_typ(tdenv: &TDEnv, typ: &ast::Typ) -> Result<ast::Typ, TypeError> {
    let TypKind::Var(id, targs) = &typ.node else {
        return Ok(typ.clone());
    };
    let Some(typdef) = tdenv.get(id) else {
        let error_kind = TypeErrorKind::UndefinedType(id.node.clone());
        let error = TypeError::new(error_kind, typ.span.clone());
        return Err(error);
    };
    let TypeDef::Defined(tparams, deftyp) = typdef else {
        return Ok(typ.clone());
    };
    let DefTypKind::Plain(typ_alias) = &deftyp.node else {
        return Ok(typ.clone());
    };
    let theta = match Theta::from_lists(tparams, targs) {
        Ok(theta) => theta,
        Err(arity_mismatch) => {
            let arity_mismatch = TypeArityMismatch::TypeArgument(arity_mismatch);
            let error_kind = TypeErrorKind::ArityMismatch(arity_mismatch);
            let error = TypeError::new(error_kind, typ.span.clone());
            return Err(error);
        }
    };
    let typ_expanded = subst_typ(&theta, typ_alias)?;
    expand_typ(tdenv, &typ_expanded)
}
