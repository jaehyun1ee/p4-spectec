//! Fresh identifiers for internal language data

use std::collections::{BTreeMap, BTreeSet};

use crate::{
    domain::source::{Region, Spanned},
    lang::xl,
};

use super::{ast::*, eq, print, var};

pub type Ids = BTreeSet<IdKind>;
pub type Metavars = BTreeMap<IdKind, Typ>;

pub fn id(ids: &Ids, id: &Id) -> Id {
    let base = xl::var::strip_var_suffix(id).node;
    let ids = ids
        .iter()
        .filter(|name| {
            xl::var::strip_var_suffix(&Spanned::new((*name).clone(), id.span.clone())).node == base
        })
        .cloned()
        .collect::<Ids>();
    let mut fresh = id.clone();
    while ids.contains(&fresh.node) {
        fresh.node.push('\'');
    }
    fresh
}

pub(crate) fn var_from_typ_with_aliases(
    aliases: &[(Id, Typ)],
    ids: &Ids,
    at: Region,
    typ: &Typ,
    wildcard: bool,
) -> Var {
    fn derive(aliases: &[(Id, Typ)], at: &Region, typ: &Typ) -> Var {
        let matching = aliases
            .iter()
            .filter(|(name, alias)| {
                eq::eq_typ(typ, alias) && print::string_of_typ(typ) != name.node
            })
            .map(|(name, alias)| (name.clone(), alias.clone(), vec![]))
            .collect::<Vec<_>>();
        if let [alias] = matching.as_slice() {
            return alias.clone();
        }
        match &typ.node {
            TypKind::IterT(inner, iter) => {
                let (id, typ, mut iters) = derive(aliases, at, inner);
                iters.push(*iter);
                (id, typ, iters)
            }
            _ => (
                Spanned::new(print::string_of_typ(typ), at.clone()),
                typ.clone(),
                vec![],
            ),
        }
    }

    let (var_id, typ, iters) = derive(aliases, &at, typ);
    let var_id = if wildcard {
        Spanned::new(format!("_{}", var_id.node), var_id.span)
    } else {
        var_id
    };
    (id(ids, &var_id), typ, iters)
}

fn aliases_at(metavars: &Metavars, at: &Region) -> Vec<(Id, Typ)> {
    metavars
        .iter()
        .map(|(name, typ)| (Spanned::new(name.clone(), at.clone()), typ.clone()))
        .collect()
}

pub fn var_from_typ(metavars: &Metavars, ids: &Ids, at: Region, typ: &Typ) -> Var {
    let aliases = aliases_at(metavars, &at);
    var_from_typ_with_aliases(&aliases, ids, at, typ, false)
}

pub fn var_from_typ_wildcard(metavars: &Metavars, ids: &Ids, at: Region, typ: &Typ) -> Var {
    let aliases = aliases_at(metavars, &at);
    var_from_typ_with_aliases(&aliases, ids, at, typ, true)
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
    let aliases = aliases_at(metavars, &typ.span);
    exp_from_typ_with_aliases(dim, &aliases, ids, typ)
}

pub(crate) fn exp_from_typ_with_aliases(
    dim: bool,
    aliases: &[(Id, Typ)],
    ids: &Ids,
    typ: &Typ,
) -> (Ids, Exp) {
    let variable = var_from_typ_with_aliases(aliases, ids, typ.span.clone(), typ, false);
    let mut ids = ids.clone();
    ids.insert(variable.0.node.clone());
    (ids, var::as_exp(&variable, dim))
}
