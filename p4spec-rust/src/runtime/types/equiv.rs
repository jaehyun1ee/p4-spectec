//! Semantic equivalence for intermediate-language types

use crate::lang::{
    common::{ds::map::ArityMismatch, source::Span},
    il::ast::{self, TypKind},
    xl::num,
};

use super::{
    Fresh, TDEnv, Theta, TypeArityMismatch, TypeDef, TypeError, TypeErrorKind, expand_typ,
    subst_typ_inner, subst_typs_inner,
};

// == Types

/// Tests type equivalence after expanding plain aliases
pub fn equiv_typ(tdenv: &TDEnv, typ_l: &ast::Typ, typ_r: &ast::Typ) -> Result<bool, TypeError> {
    let typ_l = expand_typ(tdenv, typ_l)?;
    let typ_r = expand_typ(tdenv, typ_r)?;
    match (&typ_l.node, &typ_r.node) {
        (TypKind::Bool, TypKind::Bool) | (TypKind::Text, TypKind::Text) => Ok(true),
        (TypKind::Num(num_typ_l), TypKind::Num(num_typ_r)) => {
            let equiv = num::equiv(*num_typ_l, *num_typ_r);
            Ok(equiv)
        }
        (TypKind::Var(id_l, targs_l), TypKind::Var(id_r, targs_r)) => {
            if id_l.node != id_r.node {
                return Ok(false);
            }
            equiv_typs(tdenv, targs_l, targs_r)
        }
        (TypKind::Tuple(typs_l), TypKind::Tuple(typs_r)) => equiv_typs(tdenv, typs_l, typs_r),
        (TypKind::Iter(typ_l, iter_l), TypKind::Iter(typ_r, iter_r)) => {
            if iter_l != iter_r {
                return Ok(false);
            }
            equiv_typ(tdenv, typ_l, typ_r)
        }
        _ => Ok(false),
    }
}

fn equiv_typs(tdenv: &TDEnv, typs_l: &[ast::Typ], typs_r: &[ast::Typ]) -> Result<bool, TypeError> {
    if typs_l.len() != typs_r.len() {
        return Ok(false);
    }
    for (typ_l, typ_r) in typs_l.iter().zip(typs_r) {
        if !equiv_typ(tdenv, typ_l, typ_r)? {
            return Ok(false);
        }
    }
    Ok(true)
}

// == Notation types

/// Tests notation-type equivalence
pub fn equiv_not_typ(
    tdenv: &TDEnv,
    not_typ_l: &ast::NotTyp,
    not_typ_r: &ast::NotTyp,
) -> Result<bool, TypeError> {
    let (mixop_l, typs_l) = not_typ_l.node.split();
    let (mixop_r, typs_r) = not_typ_r.node.split();
    if mixop_l != mixop_r {
        return Ok(false);
    }
    for (typ_l, typ_r) in typs_l.into_iter().zip(typs_r) {
        if !equiv_typ(tdenv, typ_l, typ_r)? {
            return Ok(false);
        }
    }
    Ok(true)
}

// == Function types

/// Tests alpha-equivalence of two function types
pub fn equiv_func_typ(
    tdenv: &TDEnv,
    span: &Span,
    func_typ_l: &ast::FuncTyp,
    func_typ_r: &ast::FuncTyp,
) -> Result<bool, TypeError> {
    let tparams_l = &func_typ_l.tparams;
    let tparams_r = &func_typ_r.tparams;
    if tparams_l.len() != tparams_r.len() {
        let mismatch = ArityMismatch::new(tparams_l.len(), tparams_r.len());
        let mismatch = TypeArityMismatch::TypeParameter(mismatch);
        let kind = TypeErrorKind::ArityMismatch(mismatch);
        let error = TypeError::new(kind, span.clone());
        return Err(error);
    }
    let typs_params_l = &func_typ_l.typs_params;
    let typs_params_r = &func_typ_r.typs_params;
    if typs_params_l.len() != typs_params_r.len() {
        let mismatch = ArityMismatch::new(typs_params_l.len(), typs_params_r.len());
        let mismatch = TypeArityMismatch::Parameter(mismatch);
        let kind = TypeErrorKind::ArityMismatch(mismatch);
        let error = TypeError::new(kind, span.clone());
        return Err(error);
    }

    let mut fresh = Fresh::default();
    let mut theta_l = Theta::new();
    let mut theta_r = Theta::new();
    let mut tdenv_fresh = tdenv.clone();
    for (tparam_l, tparam_r) in tparams_l.iter().zip(tparams_r) {
        let (tparam_fresh, typ_fresh) = fresh.fresh();
        tdenv_fresh.insert(tparam_fresh, TypeDef::Parameter);
        theta_l.insert(tparam_l.clone(), typ_fresh.clone());
        theta_r.insert(tparam_r.clone(), typ_fresh);
    }

    let typs_params_l = subst_typs_inner(&mut fresh, &theta_l, typs_params_l)?;
    let typs_params_r = subst_typs_inner(&mut fresh, &theta_r, typs_params_r)?;
    let typ_ret_l = subst_typ_inner(&mut fresh, &theta_l, &func_typ_l.typ_ret)?;
    let typ_ret_r = subst_typ_inner(&mut fresh, &theta_r, &func_typ_r.typ_ret)?;

    if !equiv_typs(&tdenv_fresh, &typs_params_l, &typs_params_r)? {
        return Ok(false);
    }
    equiv_typ(&tdenv_fresh, &typ_ret_l, &typ_ret_r)
}
