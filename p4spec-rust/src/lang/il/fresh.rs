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
            .map(|(name, alias)| Var {
                id: name.clone(),
                typ: alias.clone(),
                iters: vec![],
            })
            .collect::<Vec<_>>();
        if let [alias] = matching.as_slice() {
            return alias.clone();
        }
        match &typ.node {
            TypKind::IterT(inner, iter) => {
                let mut variable = derive(aliases, at, inner);
                variable.iters.push(*iter);
                variable
            }
            _ => Var {
                id: Spanned::new(print::string_of_typ(typ), at.clone()),
                typ: typ.clone(),
                iters: vec![],
            },
        }
    }

    let mut variable = derive(aliases, &at, typ);
    variable.id = if wildcard {
        Spanned::new(format!("_{}", variable.id.node), variable.id.span)
    } else {
        variable.id
    };
    variable.id = id(ids, &variable.id);
    variable
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
    ids.insert(variable.id.node.clone());
    (ids, var::as_exp(&variable, dim))
}
