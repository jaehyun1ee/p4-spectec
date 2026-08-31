//! Type-variable substitution for intermediate-language types and notation types
//!
//! Function-type binders are freshened before applying the outer substitution

use crate::lang::{
    common::{ds::map::IdMap, notation::mixop::Mixop},
    il::ast::{self, TypKind},
};
use crate::phrase;

use super::{Fresh, TypeError, TypeErrorKind};

pub type Theta = IdMap<ast::Typ>;

fn freshen_tparams(fresh: &mut Fresh, tparams: &[ast::TParam]) -> (Theta, Vec<ast::TParam>) {
    let mut theta = Theta::new();
    let mut tparams_fresh = Vec::with_capacity(tparams.len());
    for tparam in tparams {
        let (tparam_fresh, typ_fresh) = fresh.fresh();
        theta.insert(tparam.clone(), typ_fresh);
        tparams_fresh.push(tparam_fresh);
    }
    (theta, tparams_fresh)
}

// == Types

/// Substitutes type variables while freshening nested function binders
pub fn subst_typ(theta: &Theta, typ: &ast::Typ) -> Result<ast::Typ, TypeError> {
    let mut fresh = Fresh::default();
    subst_typ_inner(&mut fresh, theta, typ)
}

pub(crate) fn subst_typ_inner(
    fresh: &mut Fresh,
    theta: &Theta,
    typ: &ast::Typ,
) -> Result<ast::Typ, TypeError> {
    if theta.is_empty() {
        return Ok(typ.clone());
    }
    match &typ.node {
        TypKind::Bool | TypKind::Num(_) | TypKind::Text => Ok(typ.clone()),
        TypKind::Var(id, targs) => match theta.get(id) {
            Some(_) if !targs.is_empty() => {
                let error =
                    TypeError::new(TypeErrorKind::HigherOrderSubstitution, typ.span.clone());
                Err(error)
            }
            Some(typ_subst) => Ok(typ_subst.clone()),
            None => {
                let targs = subst_typs_inner(fresh, theta, targs)?;
                let typ_kind = TypKind::Var(id.clone(), targs);
                let typ_subst = phrase!(node: typ_kind, span: typ.span.clone());
                Ok(typ_subst)
            }
        },
        TypKind::Tuple(typs) => {
            let typs = subst_typs_inner(fresh, theta, typs)?;
            let typ_kind = TypKind::Tuple(typs);
            let typ_subst = phrase!(node: typ_kind, span: typ.span.clone());
            Ok(typ_subst)
        }
        TypKind::Iter(typ_inner, iter) => {
            let typ_inner = subst_typ_inner(fresh, theta, typ_inner)?;
            let span = typ_inner.span.clone();
            let typ_inner = Box::new(typ_inner);
            let typ_kind = TypKind::Iter(typ_inner, *iter);
            let typ_subst = phrase!(node: typ_kind, span: span);
            Ok(typ_subst)
        }
        TypKind::Func(func_typ) => {
            let (theta_fresh, tparams) = freshen_tparams(fresh, &func_typ.tparams);
            let typs_params = subst_typs_inner(fresh, &theta_fresh, &func_typ.typs_params)?;
            let typs_params = subst_typs_inner(fresh, theta, &typs_params)?;
            let typ_ret = subst_typ_inner(fresh, &theta_fresh, &func_typ.typ_ret)?;
            let typ_ret = subst_typ_inner(fresh, theta, &typ_ret)?;
            let typ_ret = Box::new(typ_ret);
            let func_typ = ast::FuncTyp {
                tparams,
                typs_params,
                typ_ret,
            };
            let typ_kind = TypKind::Func(func_typ);
            let typ_subst = phrase!(node: typ_kind, span: typ.span.clone());
            Ok(typ_subst)
        }
    }
}

/// Substitutes type variables in a type list
pub fn subst_typs(theta: &Theta, typs: &[ast::Typ]) -> Result<Vec<ast::Typ>, TypeError> {
    let mut fresh = Fresh::default();
    subst_typs_inner(&mut fresh, theta, typs)
}

pub(crate) fn subst_typs_inner(
    fresh: &mut Fresh,
    theta: &Theta,
    typs: &[ast::Typ],
) -> Result<Vec<ast::Typ>, TypeError> {
    let mut typs_subst = Vec::with_capacity(typs.len());
    for typ in typs {
        let typ_subst = subst_typ_inner(fresh, theta, typ)?;
        typs_subst.push(typ_subst);
    }
    Ok(typs_subst)
}

// == Notation types

/// Substitutes type variables in a notation type
pub fn subst_not_typ(theta: &Theta, not_typ: &ast::NotTyp) -> Result<ast::NotTyp, TypeError> {
    let mut fresh = Fresh::default();
    subst_not_typ_inner(&mut fresh, theta, not_typ)
}

pub(crate) fn subst_not_typ_inner(
    fresh: &mut Fresh,
    theta: &Theta,
    not_typ: &ast::NotTyp,
) -> Result<ast::NotTyp, TypeError> {
    if theta.is_empty() {
        return Ok(not_typ.clone());
    }
    let (mixop, typs) = not_typ.node.split();
    let mut typs_subst = Vec::with_capacity(typs.len());
    for typ in typs {
        let typ_subst = subst_typ_inner(fresh, theta, typ)?;
        typs_subst.push(typ_subst);
    }
    let not_typ_node = Mixop::fill(&mixop, typs_subst);
    let not_typ_node =
        not_typ_node.expect("arguments obtained from the same mixfix must match its arity");
    let not_typ_subst = phrase!(node: not_typ_node, span: not_typ.span.clone());
    Ok(not_typ_subst)
}

// == Parameters

fn subst_param_inner(
    fresh: &mut Fresh,
    theta: &Theta,
    param: &ast::Param,
) -> Result<ast::Param, TypeError> {
    let kind = match &param.node {
        ast::ParamKind::Exp(typ) => ast::ParamKind::Exp(subst_typ_inner(fresh, theta, typ)?),
        ast::ParamKind::Def(id, tparams, params, typ) => {
            let (theta_fresh, tparams) = freshen_tparams(fresh, tparams);
            let params = subst_params_inner(fresh, &theta_fresh, params)?;
            let params = subst_params_inner(fresh, theta, &params)?;
            let typ = subst_typ_inner(fresh, &theta_fresh, typ)?;
            let typ = subst_typ_inner(fresh, theta, &typ)?;
            ast::ParamKind::Def(id.clone(), tparams, params, typ)
        }
    };
    let param = crate::phrase! {
        node: kind,
        span: param.span.clone(),
    };
    Ok(param)
}

/// Substitutes type variables in parameters while sharing fresh state
pub(crate) fn subst_params(
    theta: &Theta,
    params: &[ast::Param],
) -> Result<Vec<ast::Param>, TypeError> {
    let mut fresh = Fresh::default();
    subst_params_inner(&mut fresh, theta, params)
}

fn subst_params_inner(
    fresh: &mut Fresh,
    theta: &Theta,
    params: &[ast::Param],
) -> Result<Vec<ast::Param>, TypeError> {
    params
        .iter()
        .map(|param| subst_param_inner(fresh, theta, param))
        .collect()
}
