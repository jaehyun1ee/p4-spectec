//! Expansion of intermediate-language type aliases

use std::borrow::Cow;

use crate::lang::il::ast::{self, DefTypKind, TypKind};

use super::{TDEnv, Theta, TypeArityMismatch, TypeDef, TypeError, TypeErrorKind, subst_typ};

/// Expands plain type aliases until a non-alias type is reached
pub fn expand_typ<'a>(tdenv: &TDEnv, typ: &'a ast::Typ) -> Result<Cow<'a, ast::Typ>, TypeError> {
    let find_typdef_opt = |id: &ast::Id| tdenv.get(id);
    expand_typ_with(&find_typdef_opt, typ)
}

pub(super) fn expand_typ_with<'a, 'env>(
    find_typdef_opt: &impl Fn(&ast::Id) -> Option<&'env TypeDef>,
    typ: &'a ast::Typ,
) -> Result<Cow<'a, ast::Typ>, TypeError> {
    let TypKind::Var(id, targs) = &typ.node else {
        return Ok(Cow::Borrowed(typ));
    };
    let Some(typdef) = find_typdef_opt(id) else {
        let error_kind = TypeErrorKind::UndefinedType(id.node.clone());
        let error = TypeError::new(error_kind, typ.span.clone());
        return Err(error);
    };
    let TypeDef::Defined(tparams, deftyp) = typdef else {
        return Ok(Cow::Borrowed(typ));
    };
    let DefTypKind::Plain(typ_alias) = &deftyp.node else {
        return Ok(Cow::Borrowed(typ));
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
    let typ_expanded = match expand_typ_with(find_typdef_opt, &typ_expanded)? {
        Cow::Borrowed(_) => typ_expanded,
        Cow::Owned(typ_expanded) => typ_expanded,
    };
    Ok(Cow::Owned(typ_expanded))
}
