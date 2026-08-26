//! Fresh intermediate-language variables and expressions

use std::collections::BTreeMap;

use crate::{
    domain::{
        sets::IdSet,
        source::{Span, Spanned},
    },
    lang::xl,
};

use super::{ast::*, eq, print, var};

type Metavars = BTreeMap<IdKind, Typ>;

fn id(ids: &IdSet, id: &Id) -> Id {
    let base = xl::var::strip_var_suffix(id).node;
    let ids = ids
        .iter()
        .filter(|name| {
            let id_candidate = crate::spanned! {
                node: (*name).clone(),
                span: id,
            };
            xl::var::strip_var_suffix(&id_candidate).node == base
        })
        .cloned()
        .collect::<IdSet>();
    let mut fresh = id.clone();
    while ids.contains(&fresh.node) {
        fresh.node.push('\'');
    }
    fresh
}

fn find_alias(metavars: &Metavars, span: &Span, typ: &Typ) -> Option<Var> {
    let typ_name = print::string_of_typ(typ);
    let mut matching = metavars
        .iter()
        .filter(|(name, alias)| eq::eq_typ(typ, alias) && typ_name.as_str() != name.as_str());
    let (name, alias) = matching.next()?;
    if matching.next().is_some() {
        return None;
    }
    Some(Var {
        id: Spanned::new(name.clone(), span.clone()),
        typ: alias.clone(),
        iters: vec![],
    })
}

fn var_from_typ_inner(metavars: &Metavars, span: &Span, typ: &Typ) -> Var {
    if let Some(alias) = find_alias(metavars, span, typ) {
        return alias;
    }
    match &typ.node {
        TypKind::Iter(inner, iter) => {
            let mut var = var_from_typ_inner(metavars, span, inner);
            var.iters.push(*iter);
            var
        }
        _ => Var {
            id: Spanned::new(print::string_of_typ(typ), span.clone()),
            typ: typ.clone(),
            iters: vec![],
        },
    }
}

/// Constructs a fresh variable for `typ`
pub fn var_from_typ(metavars: &Metavars, ids: &IdSet, span: Span, typ: &Typ) -> Var {
    let mut var = var_from_typ_inner(metavars, &span, typ);
    var.id = id(ids, &var.id);
    var
}

/// Constructs a fresh wildcard variable for `typ`
pub fn var_from_typ_wildcard(metavars: &Metavars, ids: &IdSet, span: Span, typ: &Typ) -> Var {
    let mut var = var_from_typ_inner(metavars, &span, typ);
    var.id.node.insert(0, '_');
    var.id = id(ids, &var.id);
    var
}

/// Constructs a fresh variable expression for `typ`
pub fn exp_from_typ(is_dim: bool, metavars: &Metavars, ids: &IdSet, typ: &Typ) -> (IdSet, Exp) {
    let var = var_from_typ(metavars, ids, typ.span.clone(), typ);
    let mut ids = ids.clone();
    ids.insert(var.id.node.clone());
    (ids, var::as_exp(is_dim, &var))
}
