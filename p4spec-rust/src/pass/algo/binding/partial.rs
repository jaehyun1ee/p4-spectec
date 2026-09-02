//! Rewriting of partially bound patterns:
//!
//! Bound values inside a binder become fresh variables plus equality premises.
//!
//! -- let PATTERN (a, 1 + 2) = ...
//!
//! becomes
//!
//! -- let PATTERN (a, int) = ..., -- if int = 1 + 2
//!
//! Variant and subtype injections become guards followed by simpler bindings.
//!
//! -- let PATTERN (a, int) = pat
//!
//! becomes
//!
//! -- if pat matches PATTERN, -- let PATTERN (a, b) = pat
//!
//! -- let ((typ) child) = parent
//!
//! becomes
//!
//! -- if parent <: child, -- let child = parent as child
//!
//! Generated premises retain the iteration context of the source pattern.

use crate::{
    lang::{
        al,
        common::{ds::set::IdSet, notation::mixop::Mixop},
        il::{ast, fresh, var},
        traits::free::Free,
        xl,
    },
    note_phrase, phrase,
    runtime::{
        sta::Dim,
        types::{Theta, TypeDef, optimize_sub_typ, subst_typ},
    },
};

use super::{
    super::{AlgoError, AlgoErrorKind},
    context::Context,
    dimension,
    iteration::{ICtx, Iteration},
};

// == Helpers

fn is_singleton_case(ctx: &Context, typ: &ast::Typ) -> Result<bool, AlgoError> {
    let ast::TypKind::Var(id, targs) = &typ.node else {
        return Ok(false);
    };
    let TypeDef::Defined(tparams, def_typ) = ctx.find_typdef(id)? else {
        return Ok(false);
    };
    match &def_typ.node {
        ast::DefTypKind::Plain(typ_inner) => {
            let theta = Theta::from_lists(tparams, targs).map_err(|mismatch| {
                AlgoError::new(
                    AlgoErrorKind::TypeArgumentArityMismatch {
                        expected: mismatch.expected,
                        actual: mismatch.actual,
                    },
                    typ.span.clone(),
                )
            })?;
            let typ_inner = subst_typ(&theta, typ_inner)?;
            is_singleton_case(ctx, &typ_inner)
        }
        ast::DefTypKind::Struct(_) => Ok(false),
        ast::DefTypKind::Variant(cases) => Ok(cases.len() == 1),
    }
}

fn is_upcast_terminal(exp: &ast::Exp) -> bool {
    matches!(
        &exp.node,
        ast::ExpKind::UpCast(_, exp_inner)
            if matches!(&exp_inner.node, ast::ExpKind::Case(not_exp) if not_exp.arity() == 0)
    )
}

// == Rename environment

#[derive(Debug)]
#[allow(clippy::large_enum_variant)]
enum Source {
    Bound {
        exp_from: ast::Exp,
    },
    BindMatch {
        pattern: ast::Pattern,
        exp_from: ast::Exp,
    },
    BindSub {
        typ_sub: ast::Typ,
        exp_sub: ast::Exp,
        exp_from: ast::Exp,
    },
}

#[derive(Debug)]
pub(crate) struct Rename {
    pub(crate) destination: ast::Var,
    source: Source,
    pub(crate) iter_ctx: ICtx,
}

#[derive(Debug)]
pub struct RenameEnv {
    pub(crate) renames: Vec<Rename>,
}

impl RenameEnv {
    pub fn new() -> Self {
        Self {
            renames: Vec::new(),
        }
    }

    fn prepend(&mut self, rename: Rename) {
        self.renames.insert(0, rename);
    }

    fn append(&mut self, mut other: Self) {
        self.renames.append(&mut other.renames);
    }
}

// == Premise generation

fn gen_prem_bound(
    ctx: &Context,
    destination: &ast::Var,
    exp_from: &ast::Exp,
    iter_ctx: &ICtx,
) -> Result<al::ast::Prem, AlgoError> {
    let exp_l = var::as_exp(true, destination);
    let kind = match &exp_from.node {
        ast::ExpKind::Case(not_exp)
            if not_exp.arity() == 0
                && !is_singleton_case(
                    ctx,
                    &phrase!(node: exp_from.note.clone(), span: exp_from.span.clone()),
                )? =>
        {
            ast::ExpKind::Match(
                Box::new(exp_l),
                ast::Pattern::Case(Box::new(not_exp.to_mixop())),
            )
        }
        ast::ExpKind::Opt(Some(_)) => {
            ast::ExpKind::Match(Box::new(exp_l), ast::Pattern::Opt(ast::OptPattern::Some))
        }
        ast::ExpKind::Opt(None) => {
            ast::ExpKind::Match(Box::new(exp_l), ast::Pattern::Opt(ast::OptPattern::None))
        }
        ast::ExpKind::List(exps) if exps.is_empty() => {
            ast::ExpKind::Match(Box::new(exp_l), ast::Pattern::List(ast::ListPattern::Nil))
        }
        _ => ast::ExpKind::Cmp(
            ast::CmpOp::Bool(xl::bool::CmpOp::Eq),
            ast::OpTyp::Bool,
            Box::new(exp_l),
            Box::new(exp_from.clone()),
        ),
    };
    let exp_cond = note_phrase! {
        node: kind,
        note: ast::TypKind::Bool,
        span: exp_from.span.clone(),
    };
    let side_condition = phrase! {
        node: al::ast::PremKind::If(al::ast::IfPrem { exp: exp_cond }),
        span: exp_from.span.clone(),
    };
    let mut iter_ctx = iter_ctx.clone();
    let venv = dimension::infer_exp(exp_from);
    iter_ctx.filter_bound(|var| {
        venv.get(&var.id)
            .is_some_and(|dim_source| dim_source.sub(&Dim::new(var.typ.clone(), var.iters.clone())))
    });
    iter_ctx.add_var_bound(
        destination.id.clone(),
        destination.typ.clone(),
        destination.iters.clone(),
    );
    let prem = iter_ctx.iterate_prem(side_condition);
    Ok(prem)
}

fn gen_prem_bind_match(
    destination: &ast::Var,
    pattern: &ast::Pattern,
    exp_from: &ast::Exp,
    iter_ctx: &ICtx,
) -> Vec<al::ast::Prem> {
    let exp_to = var::as_exp(true, destination);
    let exp_guard_match = note_phrase! {
        node: ast::ExpKind::Match(Box::new(exp_to.clone()), pattern.clone()),
        note: ast::TypKind::Bool,
        span: exp_from.span.clone(),
    };
    let side_condition_guard_match = phrase! {
        node: al::ast::PremKind::If(al::ast::IfPrem {
            exp: exp_guard_match,
        }),
        span: exp_from.span.clone(),
    };
    let mut iter_ctx_match = ICtx::from_iterations(
        iter_ctx
            .as_slice()
            .iter()
            .map(|entry| Iteration {
                iter: entry.iter,
                vars_bound: vec![],
                vars_bind: vec![],
            })
            .collect(),
    );
    iter_ctx_match.add_var_bound(
        destination.id.clone(),
        destination.typ.clone(),
        destination.iters.clone(),
    );

    let prem_bind = phrase! {
        node: al::ast::PremKind::Let(al::ast::LetPrem {
            exp_l: exp_from.clone(),
            exp_r: exp_to,
        }),
        span: exp_from.span.clone(),
    };
    let mut iter_ctx_bind = ICtx::from_iterations(
        iter_ctx
            .as_slice()
            .iter()
            .map(|entry| Iteration {
                iter: entry.iter,
                vars_bound: vec![],
                vars_bind: vec![],
            })
            .collect(),
    );
    iter_ctx_bind.add_vars_bind(dimension::infer_exp(exp_from));
    iter_ctx_bind.add_var_bound(
        destination.id.clone(),
        destination.typ.clone(),
        destination.iters.clone(),
    );
    let prem_match = iter_ctx_match.iterate_prem(side_condition_guard_match);
    let prem_bind = iter_ctx_bind.iterate_prem(prem_bind);
    vec![prem_match, prem_bind]
}

fn gen_prem_bind_sub(
    ctx: &Context,
    destination: &ast::Var,
    typ_sub: &ast::Typ,
    exp_sub: &ast::Exp,
    exp_from: &ast::Exp,
    iter_ctx: &ICtx,
) -> Result<Vec<al::ast::Prem>, AlgoError> {
    let exp_to = var::as_exp(true, destination);
    let typ_source = phrase!(node: exp_to.note.clone(), span: exp_to.span.clone());
    let subcheck = optimize_sub_typ(&ctx.tdenv, &typ_source, typ_sub)?;
    let exp_guard_sub = note_phrase! {
        node: ast::ExpKind::Sub(
            Box::new(exp_to.clone()),
            typ_sub.clone(),
            Box::new(subcheck),
        ),
        note: ast::TypKind::Bool,
        span: exp_from.span.clone(),
    };
    let side_condition_guard_sub = phrase! {
        node: al::ast::PremKind::If(al::ast::IfPrem { exp: exp_guard_sub }),
        span: exp_from.span.clone(),
    };
    let mut iter_ctx_sub = ICtx::from_iterations(
        iter_ctx
            .as_slice()
            .iter()
            .map(|entry| Iteration {
                iter: entry.iter,
                vars_bound: vec![],
                vars_bind: vec![],
            })
            .collect(),
    );
    iter_ctx_sub.add_var_bound(
        destination.id.clone(),
        destination.typ.clone(),
        destination.iters.clone(),
    );

    let exp_downcast = note_phrase! {
        node: ast::ExpKind::DownCast(typ_sub.clone(), Box::new(exp_to)),
        note: typ_sub.node.clone(),
        span: exp_from.span.clone(),
    };
    let prem_bind = phrase! {
        node: al::ast::PremKind::Let(al::ast::LetPrem {
            exp_l: exp_sub.clone(),
            exp_r: exp_downcast,
        }),
        span: exp_from.span.clone(),
    };
    let mut iter_ctx_bind = ICtx::from_iterations(
        iter_ctx
            .as_slice()
            .iter()
            .map(|entry| Iteration {
                iter: entry.iter,
                vars_bound: vec![],
                vars_bind: vec![],
            })
            .collect(),
    );
    iter_ctx_bind.add_vars_bind(dimension::infer_exp(exp_from));
    iter_ctx_bind.add_var_bound(
        destination.id.clone(),
        destination.typ.clone(),
        destination.iters.clone(),
    );
    let prem_sub = iter_ctx_sub.iterate_prem(side_condition_guard_sub);
    let prem_bind = iter_ctx_bind.iterate_prem(prem_bind);
    Ok(vec![prem_sub, prem_bind])
}

fn gen_prem(
    ctx: &Context,
    rename: &Rename,
    iter_ctx_prem: &ICtx,
) -> Result<Vec<al::ast::Prem>, AlgoError> {
    let mut iterations = rename.iter_ctx.as_slice().to_vec();
    iterations.extend(iter_ctx_prem.as_slice().iter().cloned());
    let iter_ctx = ICtx::from_iterations(iterations);
    match &rename.source {
        Source::Bound { exp_from } => {
            let prem = gen_prem_bound(ctx, &rename.destination, exp_from, &iter_ctx)?;
            Ok(vec![prem])
        }
        Source::BindMatch { pattern, exp_from } => {
            let prems = gen_prem_bind_match(&rename.destination, pattern, exp_from, &iter_ctx);
            Ok(prems)
        }
        Source::BindSub {
            typ_sub,
            exp_sub,
            exp_from,
        } => gen_prem_bind_sub(
            ctx,
            &rename.destination,
            typ_sub,
            exp_sub,
            exp_from,
            &iter_ctx,
        ),
    }
}

pub fn gen_prems(
    ctx: &Context,
    iter_ctx_prem: &ICtx,
    renv: &RenameEnv,
) -> Result<Vec<al::ast::Prem>, AlgoError> {
    let mut prems = Vec::new();
    for rename in &renv.renames {
        prems.extend(gen_prem(ctx, rename, iter_ctx_prem)?);
    }
    Ok(prems)
}

// == Expression rewriting

fn rename_exp_bind_match(
    ctx: &mut Context,
    renv: &mut RenameEnv,
    iter_ctx: &mut ICtx,
    pattern: ast::Pattern,
    exp_from: ast::Exp,
) -> ast::Exp {
    let typ = phrase!(node: exp_from.note.clone(), span: exp_from.span.clone());
    let destination = fresh::var_from_typ(&ctx.menv, &ctx.frees, exp_from.span.clone(), &typ);
    ctx.add_free(destination.id.clone());
    renv.prepend(Rename {
        destination: destination.clone(),
        source: Source::BindMatch {
            pattern,
            exp_from: exp_from.clone(),
        },
        iter_ctx: iter_ctx.clone(),
    });
    let bounds = exp_from.free();
    iter_ctx.filter_bound(|var| !bounds.contains(&var.id));
    iter_ctx.add_var_bound(
        destination.id.clone(),
        destination.typ.clone(),
        destination.iters.clone(),
    );
    var::as_exp(true, &destination)
}

fn rename_exp_bind_sub(
    ctx: &mut Context,
    renv: &mut RenameEnv,
    iter_ctx: &mut ICtx,
    typ_sub: ast::Typ,
    exp_sub: ast::Exp,
    exp_from: ast::Exp,
) -> ast::Exp {
    let typ = phrase!(node: exp_from.note.clone(), span: exp_from.span.clone());
    let destination = fresh::var_from_typ(&ctx.menv, &ctx.frees, exp_from.span.clone(), &typ);
    ctx.add_free(destination.id.clone());
    renv.prepend(Rename {
        destination: destination.clone(),
        source: Source::BindSub {
            typ_sub,
            exp_sub,
            exp_from: exp_from.clone(),
        },
        iter_ctx: iter_ctx.clone(),
    });
    let bounds = exp_from.free();
    iter_ctx.filter_bound(|var| !bounds.contains(&var.id));
    iter_ctx.add_var_bound(
        destination.id.clone(),
        destination.typ.clone(),
        destination.iters.clone(),
    );
    var::as_exp(true, &destination)
}

pub fn rename_exp(
    ctx: &mut Context,
    binds: &IdSet,
    renv: &mut RenameEnv,
    iter_ctx: &mut ICtx,
    exp: &ast::Exp,
) -> Result<ast::Exp, AlgoError> {
    let frees = exp.free();
    let has_binding = binds.iter().any(|id| frees.contains(id));
    if !has_binding && !is_upcast_terminal(exp) {
        let exp = rename_exp_bound(ctx, renv, iter_ctx, exp);
        return Ok(exp);
    }
    rename_exp_bind(ctx, binds, renv, iter_ctx, exp)
}

fn rename_exp_bound(
    ctx: &mut Context,
    renv: &mut RenameEnv,
    iter_ctx: &mut ICtx,
    exp: &ast::Exp,
) -> ast::Exp {
    let typ = phrase!(node: exp.note.clone(), span: exp.span.clone());
    let destination = fresh::var_from_typ(&ctx.menv, &ctx.frees, exp.span.clone(), &typ);
    ctx.add_free(destination.id.clone());
    renv.prepend(Rename {
        destination: destination.clone(),
        source: Source::Bound {
            exp_from: exp.clone(),
        },
        iter_ctx: iter_ctx.clone(),
    });
    let bounds = exp.free();
    iter_ctx.filter_bound(|var| !bounds.contains(&var.id));
    iter_ctx.add_var_bound(
        destination.id.clone(),
        destination.typ.clone(),
        destination.iters.clone(),
    );
    var::as_exp(true, &destination)
}

fn rename_exp_bind(
    ctx: &mut Context,
    binds: &IdSet,
    renv: &mut RenameEnv,
    iter_ctx: &mut ICtx,
    exp: &ast::Exp,
) -> Result<ast::Exp, AlgoError> {
    let span = exp.span.clone();
    let note = exp.note.clone();
    match &exp.node {
        ast::ExpKind::UpCast(typ, exp_inner) => {
            let exp_sub = rename_exp(ctx, binds, renv, iter_ctx, exp_inner)?;
            let exp_from = note_phrase! {
                node: ast::ExpKind::UpCast(typ.clone(), Box::new(exp_sub.clone())),
                note: note,
                span: span.clone(),
            };
            let typ_sub = phrase!(node: exp_sub.note.clone(), span: span);
            let exp = rename_exp_bind_sub(ctx, renv, iter_ctx, typ_sub, exp_sub, exp_from);
            Ok(exp)
        }
        ast::ExpKind::Tuple(exps) => {
            let exps = rename_exps(ctx, binds, renv, iter_ctx, exps)?;
            let exp = note_phrase!(node: ast::ExpKind::Tuple(exps), note: note, span: span);
            Ok(exp)
        }
        ast::ExpKind::Case(not_exp) => {
            let args = not_exp.args().into_iter().cloned().collect::<Vec<_>>();
            let mixop = not_exp.to_mixop();
            let args = rename_exps(ctx, binds, renv, iter_ctx, &args)?;
            let not_exp = Mixop::fill(&mixop, args)
                .expect("arguments obtained from the same mixfix must match its arity");
            let exp_from = note_phrase! {
                node: ast::ExpKind::Case(Box::new(not_exp)),
                note: note.clone(),
                span: span.clone(),
            };
            let typ = phrase!(node: note, span: span.clone());
            if is_singleton_case(ctx, &typ)? {
                Ok(exp_from)
            } else {
                let pattern = ast::Pattern::Case(Box::new(mixop));
                let exp = rename_exp_bind_match(ctx, renv, iter_ctx, pattern, exp_from);
                Ok(exp)
            }
        }
        ast::ExpKind::Str(fields) => {
            let exps = fields
                .iter()
                .map(|(_, exp)| exp.clone())
                .collect::<Vec<_>>();
            let exps = rename_exps(ctx, binds, renv, iter_ctx, &exps)?;
            let fields = fields
                .iter()
                .map(|(atom, _)| atom.clone())
                .zip(exps)
                .collect();
            let exp = note_phrase!(node: ast::ExpKind::Str(fields), note: note, span: span);
            Ok(exp)
        }
        ast::ExpKind::Opt(Some(exp_inner)) => {
            let exp_inner = rename_exp(ctx, binds, renv, iter_ctx, exp_inner)?;
            let exp_from = note_phrase! {
                node: ast::ExpKind::Opt(Some(Box::new(exp_inner))),
                note: note,
                span: span,
            };
            let pattern = ast::Pattern::Opt(ast::OptPattern::Some);
            let exp = rename_exp_bind_match(ctx, renv, iter_ctx, pattern, exp_from);
            Ok(exp)
        }
        ast::ExpKind::Opt(None) => {
            let pattern = ast::Pattern::Opt(ast::OptPattern::None);
            let exp_from = exp.clone();
            let exp = rename_exp_bind_match(ctx, renv, iter_ctx, pattern, exp_from);
            Ok(exp)
        }
        ast::ExpKind::List(exps) => {
            let exps = rename_exps(ctx, binds, renv, iter_ctx, exps)?;
            let length = i64::try_from(exps.len()).expect("expression list length fits i64");
            let exp_from = note_phrase! {
                node: ast::ExpKind::List(exps),
                note: note,
                span: span.clone(),
            };
            let pattern = if length == 0 {
                ast::ListPattern::Nil
            } else {
                ast::ListPattern::Fixed(length)
            };
            let pattern = ast::Pattern::List(pattern);
            let exp = rename_exp_bind_match(ctx, renv, iter_ctx, pattern, exp_from);
            Ok(exp)
        }
        ast::ExpKind::Cons(exp_h, exp_t) => {
            let exp_h = rename_exp(ctx, binds, renv, iter_ctx, exp_h)?;
            let exp_t = rename_exp(ctx, binds, renv, iter_ctx, exp_t)?;
            let exp_from = note_phrase! {
                node: ast::ExpKind::Cons(Box::new(exp_h), Box::new(exp_t)),
                note: note,
                span: span,
            };
            let pattern = ast::Pattern::List(ast::ListPattern::Cons);
            let exp = rename_exp_bind_match(ctx, renv, iter_ctx, pattern, exp_from);
            Ok(exp)
        }
        ast::ExpKind::Iter(exp_inner, (iter, vars)) => {
            let iteration = Iteration {
                iter: *iter,
                vars_bound: vars.clone(),
                vars_bind: vec![],
            };
            let mut iter_scope = iter_ctx.scope(iteration);
            let exp_inner = rename_exp(ctx, binds, renv, &mut iter_scope, exp_inner)?;
            let iteration = iter_scope.finish();
            let exp = note_phrase! {
                node: ast::ExpKind::Iter(
                    Box::new(exp_inner),
                    (iteration.iter, iteration.vars_bound),
                ),
                note: note,
                span: span,
            };
            Ok(exp)
        }
        _ => Ok(exp.clone()),
    }
}

pub fn rename_exps(
    ctx: &mut Context,
    binds: &IdSet,
    renv: &mut RenameEnv,
    iter_ctx: &mut ICtx,
    exps: &[ast::Exp],
) -> Result<Vec<ast::Exp>, AlgoError> {
    let mut exps_renamed = Vec::with_capacity(exps.len());
    for exp in exps {
        let mut renv_post = RenameEnv::new();
        let exp = rename_exp(ctx, binds, &mut renv_post, iter_ctx, exp)?;
        renv.append(renv_post);
        exps_renamed.push(exp);
    }
    Ok(exps_renamed)
}

// == Argument rewriting

fn rename_arg(
    ctx: &mut Context,
    binds: &IdSet,
    renv: &mut RenameEnv,
    iter_ctx: &mut ICtx,
    arg: &ast::Arg,
) -> Result<ast::Arg, AlgoError> {
    let ast::ArgKind::Exp(exp) = &arg.node else {
        return Ok(arg.clone());
    };
    let mut renv_post = RenameEnv::new();
    let exp = rename_exp(ctx, binds, &mut renv_post, iter_ctx, exp)?;
    renv.append(renv_post);
    let arg = phrase!(node: ast::ArgKind::Exp(Box::new(exp)), span: arg.span.clone());
    Ok(arg)
}

pub fn rename_args(
    ctx: &mut Context,
    binds: &IdSet,
    renv: &mut RenameEnv,
    iter_ctx: &mut ICtx,
    args: &[ast::Arg],
) -> Result<Vec<ast::Arg>, AlgoError> {
    let mut args_renamed = Vec::with_capacity(args.len());
    for arg in args {
        let arg = rename_arg(ctx, binds, renv, iter_ctx, arg)?;
        args_renamed.push(arg);
    }
    Ok(args_renamed)
}
