//! Type-variable substitution for intermediate-language types and notation types
//!
//! Function-type binders are freshened before applying the outer substitution

use std::borrow::Cow;

use crate::lang::{
    common::{
        ds::map::{ArityMismatch, IdMap},
        notation::mixop::Mixop,
    },
    il::ast::{self, TypKind},
};
use crate::phrase;

use super::{TypeError, TypeErrorKind};
use crate::runtime::value::Fresh;

#[derive(Default)]
pub struct Theta(IdMap<ast::Typ>);

impl Theta {
    pub fn new() -> Self {
        Self(IdMap::new())
    }

    pub fn from_lists(tparams: &[ast::TParam], targs: &[ast::Typ]) -> Result<Self, ArityMismatch> {
        let theta = IdMap::from_lists(tparams, targs)?;
        Ok(Self(theta))
    }

    pub fn insert(&mut self, tparam: ast::TParam, targ: ast::Typ) -> Option<ast::Typ> {
        self.0.insert(tparam, targ)
    }

    fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    fn get(&self, tparam: &ast::TParam) -> Option<&ast::Typ> {
        self.0.get(tparam)
    }
}

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
    Ok(subst_typ_cow_inner(fresh, theta, typ)?.into_owned())
}

fn subst_typ_cow_inner<'a>(
    fresh: &mut Fresh,
    theta: &Theta,
    typ: &'a ast::Typ,
) -> Result<Cow<'a, ast::Typ>, TypeError> {
    if theta.is_empty() {
        return Ok(Cow::Borrowed(typ));
    }
    match &typ.node {
        TypKind::Bool | TypKind::Num(_) | TypKind::Text => Ok(Cow::Borrowed(typ)),
        TypKind::Var(id, targs) => match theta.get(id) {
            Some(_) if !targs.is_empty() => {
                let error =
                    TypeError::new(TypeErrorKind::HigherOrderSubstitution, typ.span.clone());
                Err(error)
            }
            Some(typ_subst) => Ok(Cow::Owned(typ_subst.clone())),
            None => {
                let targs = subst_typs_cow_inner(fresh, theta, targs)?;
                let Cow::Owned(targs) = targs else {
                    return Ok(Cow::Borrowed(typ));
                };
                let typ_kind = TypKind::Var(id.clone(), targs);
                let typ_subst = phrase!(node: typ_kind, span: typ.span.clone());
                Ok(Cow::Owned(typ_subst))
            }
        },
        TypKind::Tuple(typs) => {
            let typs = subst_typs_cow_inner(fresh, theta, typs)?;
            let Cow::Owned(typs) = typs else {
                return Ok(Cow::Borrowed(typ));
            };
            let typ_kind = TypKind::Tuple(typs);
            let typ_subst = phrase!(node: typ_kind, span: typ.span.clone());
            Ok(Cow::Owned(typ_subst))
        }
        TypKind::Iter(typ_inner, iter) => {
            let typ_inner = subst_typ_cow_inner(fresh, theta, typ_inner)?;
            let Cow::Owned(typ_inner) = typ_inner else {
                return Ok(Cow::Borrowed(typ));
            };
            let span = typ_inner.span.clone();
            let typ_inner = Box::new(typ_inner);
            let typ_kind = TypKind::Iter(typ_inner, *iter);
            let typ_subst = phrase!(node: typ_kind, span: span);
            Ok(Cow::Owned(typ_subst))
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
            Ok(Cow::Owned(typ_subst))
        }
    }
}

fn subst_typs_cow_inner<'a>(
    fresh: &mut Fresh,
    theta: &Theta,
    typs: &'a [ast::Typ],
) -> Result<Cow<'a, [ast::Typ]>, TypeError> {
    let mut typs_subst: Option<Vec<ast::Typ>> = None;
    for (index, typ) in typs.iter().enumerate() {
        match subst_typ_cow_inner(fresh, theta, typ)? {
            Cow::Borrowed(_) => {
                if let Some(typs_subst) = &mut typs_subst {
                    typs_subst.push(typ.clone());
                }
            }
            Cow::Owned(typ_subst) => {
                let typs_subst = typs_subst.get_or_insert_with(|| typs[..index].to_vec());
                typs_subst.push(typ_subst);
            }
        }
    }
    match typs_subst {
        Some(typs_subst) => Ok(Cow::Owned(typs_subst)),
        None => Ok(Cow::Borrowed(typs)),
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
    Ok(subst_typs_cow_inner(fresh, theta, typs)?.into_owned())
}

// == Notation types

/// Substitutes type variables in a notation type
pub fn subst_not_typ(theta: &Theta, not_typ: &ast::NotTyp) -> Result<ast::NotTyp, TypeError> {
    let mut fresh = Fresh::default();
    Ok(subst_not_typ_inner(&mut fresh, theta, not_typ)?.into_owned())
}

pub(crate) fn subst_not_typ_inner<'a>(
    fresh: &mut Fresh,
    theta: &Theta,
    not_typ: &'a ast::NotTyp,
) -> Result<Cow<'a, ast::NotTyp>, TypeError> {
    if theta.is_empty() {
        return Ok(Cow::Borrowed(not_typ));
    }
    let typs = not_typ.node.args();
    let mut typs_subst = Vec::with_capacity(typs.len());
    let mut changed = false;
    for typ in typs {
        let typ_subst = subst_typ_cow_inner(fresh, theta, typ)?;
        changed |= matches!(typ_subst, Cow::Owned(_));
        typs_subst.push(typ_subst);
    }
    if !changed {
        return Ok(Cow::Borrowed(not_typ));
    }
    let mixop = not_typ.node.to_mixop();
    let typs_subst = typs_subst.into_iter().map(Cow::into_owned);
    let not_typ_node = Mixop::fill(&mixop, typs_subst);
    let not_typ_node =
        not_typ_node.expect("arguments obtained from the same mixfix must match its arity");
    let not_typ_subst = phrase!(node: not_typ_node, span: not_typ.span.clone());
    Ok(Cow::Owned(not_typ_subst))
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
