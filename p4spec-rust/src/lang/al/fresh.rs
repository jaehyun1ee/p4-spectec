//! Fresh algorithmic-language expressions

use crate::{
    lang::{
        common::{ds::set::IdSet, source::Span},
        il,
        traits::{eq::SyntaxEq, print::Print},
    },
    runtime::envs::algo::MEnv,
};

use super::{ast::*, var};

fn find_alias(menv: &MEnv, typ: &Typ) -> Option<Var> {
    let typ_name = Print::to_string(typ);
    let mut vars_alias = menv.iter().filter(|(id_alias, typ_alias)| {
        typ.syntax_eq(typ_alias) && typ_name.as_str() != id_alias.node.as_str()
    });
    let (id_alias, typ_alias) = vars_alias.next()?;
    if vars_alias.next().is_some() {
        return None;
    }
    Some(Var {
        id: id_alias.clone(),
        typ: typ_alias.clone(),
        iters: vec![],
    })
}

fn var_from_typ_inner(menv: &MEnv, span: &Span, typ: &Typ) -> Var {
    if let Some(var_alias) = find_alias(menv, typ) {
        return var_alias;
    }
    match &typ.node {
        TypKind::Iter(typ_inner, iter) => {
            let mut var = var_from_typ_inner(menv, span, typ_inner);
            var.iters.push(*iter);
            var
        }
        _ => Var {
            id: crate::phrase! {
                node: Print::to_string(typ),
                span: span.clone(),
            },
            typ: typ.clone(),
            iters: vec![],
        },
    }
}

fn var_from_typ(menv: &MEnv, ids: &IdSet, typ: &Typ) -> Var {
    let mut var = var_from_typ_inner(menv, &typ.span, typ);
    var.id = il::fresh::id(ids, &var.id);
    var
}

/// Constructs a fresh variable expression for `typ`
pub fn exp_from_typ(is_dim: bool, menv: &MEnv, ids: &IdSet, typ: &Typ) -> (IdSet, Exp) {
    let var = var_from_typ(menv, ids, typ);
    let mut ids_fresh = ids.clone();
    ids_fresh.insert(var.id.clone());
    (ids_fresh, var::as_exp(is_dim, &var))
}
