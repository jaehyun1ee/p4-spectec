//! Fresh intermediate-language variables and expressions

use crate::lang::{
    common::{
        ds::{map::IdMap, set::IdSet},
        source::{Span, Spanned},
    },
    traits::{eq::SyntaxEq, print::Print},
    xl,
};

use super::{ast::*, var};

type Metavars = IdMap<Typ>;

fn id(ids: &IdSet, id: &Id) -> Id {
    let base = xl::var::strip_var_suffix(id).node;
    let ids = ids
        .iter()
        .filter(|id_other| xl::var::strip_var_suffix(id_other).node == base)
        .cloned()
        .collect::<IdSet>();
    let mut fresh = id.clone();
    while ids.contains(&fresh) {
        fresh.node.push('\'');
    }
    fresh
}

fn find_alias(metavars: &Metavars, span: &Span, typ: &Typ) -> Option<Var> {
    let typ_name = Print::to_string(typ);
    let mut matching = metavars.iter().filter(|(id_alias, typ_alias)| {
        typ.syntax_eq(typ_alias) && typ_name.as_str() != id_alias.node.as_str()
    });
    let (id_alias, typ_alias) = matching.next()?;
    if matching.next().is_some() {
        return None;
    }
    Some(Var {
        id: Spanned::new(id_alias.node.clone(), span.clone()),
        typ: typ_alias.clone(),
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
            id: Spanned::new(Print::to_string(typ), span.clone()),
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
    ids.insert(var.id.clone());
    (ids, var::as_exp(is_dim, &var))
}
