//! Fresh identifiers for algorithmic language data

use std::collections::BTreeMap;

use crate::{
    domain::source::{Region, Spanned},
    lang::il,
};

use super::ast::*;

pub use il::fresh::Ids;

pub type Metavars = BTreeMap<IdKind, (Region, Typ)>;

fn aliases(metavars: &Metavars) -> Vec<(Id, Typ)> {
    metavars
        .iter()
        .map(|(name, (at, typ))| (Spanned::new(name.clone(), at.clone()), typ.clone()))
        .collect()
}

pub fn id(ids: &Ids, id: &Id) -> Id {
    il::fresh::id(ids, id)
}

pub fn var_from_typ(metavars: &Metavars, ids: &Ids, at: Region, typ: &Typ) -> Var {
    il::fresh::var_from_typ_with_aliases(&aliases(metavars), ids, at, typ, false)
}

pub fn var_from_typ_wildcard(metavars: &Metavars, ids: &Ids, at: Region, typ: &Typ) -> Var {
    il::fresh::var_from_typ_with_aliases(&aliases(metavars), ids, at, typ, true)
}

pub fn var_from_exp(metavars: &Metavars, ids: &Ids, exp: &Exp) -> Var {
    var_from_typ(
        metavars,
        ids,
        exp.span.clone(),
        &Spanned::new(exp.ty.clone(), exp.span.clone()),
    )
}

pub fn var_from_exp_wildcard(metavars: &Metavars, ids: &Ids, exp: &Exp) -> Var {
    var_from_typ_wildcard(
        metavars,
        ids,
        exp.span.clone(),
        &Spanned::new(exp.ty.clone(), exp.span.clone()),
    )
}

pub fn exp_from_typ(dim: bool, metavars: &Metavars, ids: &Ids, typ: &Typ) -> (Ids, Exp) {
    il::fresh::exp_from_typ_with_aliases(dim, &aliases(metavars), ids, typ)
}
