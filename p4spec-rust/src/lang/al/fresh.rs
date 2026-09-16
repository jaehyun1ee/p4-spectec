//! Fresh algorithmic-language expressions
//!
//! With a single alias `flag : bool`, `bool` becomes `flag` (or `flag'` if
//! `flag` is already used), and `bool?` adds an optional iteration around it

use crate::{
    lang::{
        common::{ds::set::IdSet, source::Span},
        il,
        traits::{eq::SyntaxEq, print::Print},
    },
    runtime::envs::algo::MEnv,
};

use super::{ast::*, var};

// == Expressions

/// Constructs a fresh variable expression for `typ`
pub fn exp_from_typ(is_dim: bool, menv: &MEnv, ids: &IdSet, typ: &Typ) -> (IdSet, Exp) {
    let mut var = var_from_typ(menv, &typ.span, typ);
    var.id = il::fresh::id(ids, &var.id);
    let mut ids_fresh = ids.clone();
    ids_fresh.insert(var.id.clone());
    let exp = var::as_exp(is_dim, &var);
    (ids_fresh, exp)
}

// == Variables

fn var_from_typ(menv: &MEnv, span: &Span, typ: &Typ) -> Var {
    let typ_name = Print::to_string(typ);
    let mut vars_alias = menv.iter().filter(|(id_alias, typ_alias)| {
        typ.syntax_eq(typ_alias) && typ_name.as_str() != id_alias.node.as_str()
    });
    if let (Some((id_alias, typ_alias)), None) = (vars_alias.next(), vars_alias.next()) {
        return Var {
            id: id_alias.clone(),
            typ: typ_alias.clone(),
            iters: vec![],
        };
    }

    match &typ.node {
        TypKind::Iter(typ_inner, iter) => {
            let mut var = var_from_typ(menv, span, typ_inner);
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
