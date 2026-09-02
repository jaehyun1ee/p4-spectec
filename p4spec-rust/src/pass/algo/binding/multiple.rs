//! Rewriting of repeated binding occurrences

use crate::{
    lang::{
        al,
        common::{
            Id,
            ds::{map::IdMap, set::IdSet},
        },
        il::ast,
        traits::free::Free,
        xl,
    },
    note_phrase, phrase,
    runtime::sta::{Dim, VEnv},
};

use super::{
    bind::{BEnv, Binding},
    context::Context,
    iteration::{ICtx, Iteration},
};

/// Ordered renamed occurrences for each repeated source identifier
#[derive(Clone, Debug, Default)]
pub struct RenameEnv {
    renames: IdMap<Vec<Id>>,
    dimensions: IdMap<Dim>,
}

impl RenameEnv {
    pub fn from_bindings(benv: &BEnv) -> Self {
        let mut renames = IdMap::new();
        let mut dimensions = IdMap::new();
        for (id, binding) in benv.iter() {
            let Binding::Multiple(dim) = binding else {
                continue;
            };
            renames.insert(id.clone(), Vec::new());
            dimensions.insert(id.clone(), dim.clone());
        }
        Self {
            renames,
            dimensions,
        }
    }

    pub fn iter(&self) -> impl Iterator<Item = (&Id, &Vec<Id>)> {
        self.renames.iter()
    }

    fn get(&self, id: &Id) -> Option<&Vec<Id>> {
        self.renames.get(id)
    }

    fn get_mut(&mut self, id: &Id) -> Option<&mut Vec<Id>> {
        self.renames.get_mut(id)
    }

    fn dimension(&self, id: &Id) -> Option<&Dim> {
        self.dimensions.get(id)
    }
}

fn fresh_id(ids: &IdSet, id: &Id) -> Id {
    let base = xl::var::strip_var_suffix(id).node;
    let ids_same_base = ids
        .iter()
        .filter(|id_other| xl::var::strip_var_suffix(id_other).node == base)
        .cloned()
        .collect::<IdSet>();
    let mut id_fresh = id.clone();
    while ids_same_base.contains(&id_fresh) {
        id_fresh.node.push('\'');
    }
    id_fresh
}

fn rename_var(ctx: &mut Context, renv: &mut RenameEnv, exp: &ast::Exp, id: &Id) -> ast::Exp {
    let Some(ids_rename) = renv.get_mut(id) else {
        return exp.clone();
    };
    let id_rename = if ids_rename.is_empty() {
        id.clone()
    } else {
        fresh_id(&ctx.frees, id)
    };
    ctx.add_free(id_rename.clone());
    ids_rename.push(id_rename.clone());
    note_phrase! {
        node: ast::ExpKind::Var(id_rename),
        note: exp.note.clone(),
        span: exp.span.clone(),
    }
}

pub fn rename_exp(ctx: &mut Context, renv: &mut RenameEnv, exp: &ast::Exp) -> ast::Exp {
    let kind = match &exp.node {
        ast::ExpKind::Var(id) => return rename_var(ctx, renv, exp, id),
        ast::ExpKind::UpCast(typ, exp_inner) => {
            let exp_inner = rename_exp(ctx, renv, exp_inner);
            ast::ExpKind::UpCast(typ.clone(), Box::new(exp_inner))
        }
        ast::ExpKind::Tuple(exps) => ast::ExpKind::Tuple(rename_exps(ctx, renv, exps)),
        ast::ExpKind::Case(not_exp) => {
            let not_exp = not_exp.map(|exp| rename_exp(ctx, renv, exp));
            ast::ExpKind::Case(Box::new(not_exp))
        }
        ast::ExpKind::Str(fields) => {
            let fields = fields
                .iter()
                .map(|(atom, exp)| (atom.clone(), rename_exp(ctx, renv, exp)))
                .collect();
            ast::ExpKind::Str(fields)
        }
        ast::ExpKind::Opt(Some(exp_inner)) => {
            let exp_inner = rename_exp(ctx, renv, exp_inner);
            ast::ExpKind::Opt(Some(Box::new(exp_inner)))
        }
        ast::ExpKind::Opt(None) => return exp.clone(),
        ast::ExpKind::List(exps) => ast::ExpKind::List(rename_exps(ctx, renv, exps)),
        ast::ExpKind::Cons(exp_h, exp_t) => {
            let exp_h = rename_exp(ctx, renv, exp_h);
            let exp_t = rename_exp(ctx, renv, exp_t);
            ast::ExpKind::Cons(Box::new(exp_h), Box::new(exp_t))
        }
        ast::ExpKind::Iter(exp_inner, (iter, vars)) => {
            let exp_inner = rename_exp(ctx, renv, exp_inner);
            let frees = exp_inner.free();
            let mut vars_renamed = Vec::new();
            for var in vars {
                match renv.get(&var.id) {
                    None => vars_renamed.push(var.clone()),
                    Some(ids_rename) if ids_rename.is_empty() => vars_renamed.push(var.clone()),
                    Some(ids_rename) => {
                        vars_renamed.extend(
                            ids_rename
                                .iter()
                                .filter(|id_rename| frees.contains(id_rename))
                                .map(|id_rename| ast::Var {
                                    id: id_rename.clone(),
                                    typ: var.typ.clone(),
                                    iters: var.iters.clone(),
                                }),
                        );
                    }
                }
            }
            ast::ExpKind::Iter(Box::new(exp_inner), (*iter, vars_renamed))
        }
        _ => return exp.clone(),
    };
    note_phrase!(node: kind, note: exp.note.clone(), span: exp.span.clone())
}

pub fn rename_exps(ctx: &mut Context, renv: &mut RenameEnv, exps: &[ast::Exp]) -> Vec<ast::Exp> {
    exps.iter().map(|exp| rename_exp(ctx, renv, exp)).collect()
}

pub fn rename_arg(ctx: &mut Context, renv: &mut RenameEnv, arg: &ast::Arg) -> ast::Arg {
    let ast::ArgKind::Exp(exp) = &arg.node else {
        return arg.clone();
    };
    let exp = rename_exp(ctx, renv, exp);
    phrase!(node: ast::ArgKind::Exp(Box::new(exp)), span: arg.span.clone())
}

pub fn rename_args(ctx: &mut Context, renv: &mut RenameEnv, args: &[ast::Arg]) -> Vec<ast::Arg> {
    args.iter().map(|arg| rename_arg(ctx, renv, arg)).collect()
}

fn variable_exp(id: &Id, typ: &ast::Typ, span: &crate::lang::common::source::Span) -> ast::Exp {
    note_phrase! {
        node: ast::ExpKind::Var(id.clone()),
        note: typ.node.clone(),
        span: span.clone(),
    }
}

fn equality_exp(id: &Id, id_rename: &Id, typ: &ast::Typ) -> ast::Exp {
    let exp_l = variable_exp(id, typ, &id.span);
    let exp_r = variable_exp(id_rename, typ, &id.span);
    note_phrase! {
        node: ast::ExpKind::Cmp(
            ast::CmpOp::Bool(xl::bool::CmpOp::Eq),
            ast::OpTyp::Bool,
            Box::new(exp_l),
            Box::new(exp_r),
        ),
        note: ast::TypKind::Bool,
        span: id.span.clone(),
    }
}

fn generate_side_condition(
    dim: &Dim,
    iterctx: &ICtx,
    id: &Id,
    ids_rename: &[Id],
) -> Option<al::ast::Prem> {
    let mut ids_repeated = ids_rename.iter().skip(1);
    let id_rename = ids_repeated.next()?;
    let mut id_condition = id.clone();
    id_condition.span = ids_rename.last()?.span.clone();
    let mut exp = equality_exp(&id_condition, id_rename, &dim.typ);
    for id_rename in ids_repeated {
        let exp_r = equality_exp(&id_condition, id_rename, &dim.typ);
        exp = note_phrase! {
            node: ast::ExpKind::Bin(
                ast::BinOp::Bool(xl::bool::BinOp::And),
                ast::OpTyp::Bool,
                Box::new(exp),
                Box::new(exp_r),
            ),
            note: ast::TypKind::Bool,
            span: id_condition.span.clone(),
        };
    }
    let prem = phrase! {
        node: al::ast::PremKind::If(al::ast::IfPrem { exp }),
        span: id_condition.span.clone(),
    };

    let mut iterations = dim.iters.clone();
    iterations.extend(iterctx.iters());
    let mut side_iterctx = ICtx::from_iterations(
        iterations
            .into_iter()
            .map(|iter| Iteration {
                iter,
                vars_bound: vec![],
                vars_bind: vec![],
            })
            .collect(),
    );
    let venv = std::iter::once(&id_condition)
        .chain(ids_rename)
        .map(|id| (id.clone(), Dim::new(dim.typ.clone(), vec![])))
        .collect::<VEnv>();
    side_iterctx.add_vars_bound(venv);
    Some(side_iterctx.iterate_prem(prem))
}

pub fn generate_side_conditions(iterctx: &ICtx, renv: &RenameEnv) -> Vec<al::ast::Prem> {
    renv.iter()
        .filter_map(|(id, ids_rename)| {
            let dim = renv.dimension(id)?;
            generate_side_condition(dim, iterctx, id, ids_rename)
        })
        .collect()
}
