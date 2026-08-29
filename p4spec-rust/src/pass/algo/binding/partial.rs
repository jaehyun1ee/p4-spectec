//! Rewriting of partially bound patterns

use crate::{
    lang::{
        common::{
            ds::set::IdSet,
            notation::mixop::Mixop,
            noted::Noted,
            source::{Span, Spanned},
        },
        il::{ast, fresh, var},
        traits::free::Free,
        xl,
    },
    runtime::{
        sta::{Dim, VEnv},
        types::{Theta, TypeDef, optimize_sub_typ, subst_typ},
    },
};

use super::{
    super::{AlgoError, AlgoErrorKind},
    context::Context,
    dimension,
    iteration::{Iteration, IterationContext},
};

#[derive(Clone, Debug)]
pub enum Source {
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

#[derive(Clone, Debug)]
pub struct Rename {
    pub destination: ast::Var,
    pub source: Source,
    pub iterctx: IterationContext,
}

#[derive(Clone, Debug, Default)]
pub struct RenameEnv {
    renames: Vec<Rename>,
}

impl RenameEnv {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn iter(&self) -> impl Iterator<Item = &Rename> {
        self.renames.iter()
    }

    fn add_front(&mut self, rename: Rename) {
        self.renames.insert(0, rename);
    }

    fn append(&mut self, mut other: Self) {
        self.renames.append(&mut other.renames);
    }
}

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

fn var_from_exp(ctx: &mut Context, exp: &ast::Exp) -> ast::Var {
    let typ = Spanned::new(exp.node.note.clone(), exp.span.clone());
    let destination = fresh::var_from_typ(&ctx.menv, &ctx.frees, exp.span.clone(), &typ);
    ctx.add_free(destination.id.clone());
    destination
}

fn add_destination(iterctx: &mut IterationContext, destination: &ast::Var, exp_from: &ast::Exp) {
    let bounds = exp_from.free();
    iterctx.filter_bound(|var| !bounds.contains(&var.id));
    iterctx.add_var_bound(
        destination.id.clone(),
        destination.typ.clone(),
        destination.iters.clone(),
    );
}

fn rename_bound(
    ctx: &mut Context,
    renv: &mut RenameEnv,
    mut iterctx: IterationContext,
    exp: &ast::Exp,
) -> (IterationContext, ast::Exp) {
    let destination = var_from_exp(ctx, exp);
    renv.add_front(Rename {
        destination: destination.clone(),
        source: Source::Bound {
            exp_from: exp.clone(),
        },
        iterctx: iterctx.clone(),
    });
    add_destination(&mut iterctx, &destination, exp);
    let exp = var::as_exp(true, &destination);
    (iterctx, exp)
}

fn rename_bind_match(
    ctx: &mut Context,
    renv: &mut RenameEnv,
    mut iterctx: IterationContext,
    pattern: ast::Pattern,
    exp_from: ast::Exp,
) -> (IterationContext, ast::Exp) {
    let destination = var_from_exp(ctx, &exp_from);
    renv.add_front(Rename {
        destination: destination.clone(),
        source: Source::BindMatch {
            pattern,
            exp_from: exp_from.clone(),
        },
        iterctx: iterctx.clone(),
    });
    add_destination(&mut iterctx, &destination, &exp_from);
    let exp = var::as_exp(true, &destination);
    (iterctx, exp)
}

fn rename_bind_sub(
    ctx: &mut Context,
    renv: &mut RenameEnv,
    mut iterctx: IterationContext,
    typ_sub: ast::Typ,
    exp_sub: ast::Exp,
    exp_from: ast::Exp,
) -> (IterationContext, ast::Exp) {
    let destination = var_from_exp(ctx, &exp_from);
    renv.add_front(Rename {
        destination: destination.clone(),
        source: Source::BindSub {
            typ_sub,
            exp_sub,
            exp_from: exp_from.clone(),
        },
        iterctx: iterctx.clone(),
    });
    add_destination(&mut iterctx, &destination, &exp_from);
    let exp = var::as_exp(true, &destination);
    (iterctx, exp)
}

fn has_binding(binds: &IdSet, exp: &ast::Exp) -> bool {
    let frees = exp.free();
    binds.iter().any(|id| frees.contains(id))
}

fn is_upcast_terminal(exp: &ast::Exp) -> bool {
    matches!(
        &exp.node.kind,
        ast::ExpKind::UpCast(_, exp_inner)
            if matches!(&exp_inner.node.kind, ast::ExpKind::Case(not_exp) if not_exp.arity() == 0)
    )
}

pub fn rename_exp(
    ctx: &mut Context,
    binds: &IdSet,
    renv: &mut RenameEnv,
    iterctx: IterationContext,
    exp: &ast::Exp,
) -> Result<(IterationContext, ast::Exp), AlgoError> {
    let mut ctx_next = ctx.clone();
    let mut renv_next = renv.clone();
    let result = rename_exp_in_place(&mut ctx_next, binds, &mut renv_next, iterctx, exp)?;
    *ctx = ctx_next;
    *renv = renv_next;
    Ok(result)
}

fn rename_exp_in_place(
    ctx: &mut Context,
    binds: &IdSet,
    renv: &mut RenameEnv,
    iterctx: IterationContext,
    exp: &ast::Exp,
) -> Result<(IterationContext, ast::Exp), AlgoError> {
    if !has_binding(binds, exp) && !is_upcast_terminal(exp) {
        return Ok(rename_bound(ctx, renv, iterctx, exp));
    }
    rename_binding_exp(ctx, binds, renv, iterctx, exp)
}

fn rename_binding_exp(
    ctx: &mut Context,
    binds: &IdSet,
    renv: &mut RenameEnv,
    iterctx: IterationContext,
    exp: &ast::Exp,
) -> Result<(IterationContext, ast::Exp), AlgoError> {
    let span = exp.span.clone();
    let note = exp.node.note.clone();
    match &exp.node.kind {
        ast::ExpKind::UpCast(typ, exp_inner) => {
            let (iterctx, exp_sub) = rename_exp_in_place(ctx, binds, renv, iterctx, exp_inner)?;
            let exp_from = Spanned::new(
                Noted::new(
                    ast::ExpKind::UpCast(typ.clone(), Box::new(exp_sub.clone())),
                    note,
                ),
                span.clone(),
            );
            let typ_sub = Spanned::new(exp_sub.node.note.clone(), span);
            Ok(rename_bind_sub(
                ctx, renv, iterctx, typ_sub, exp_sub, exp_from,
            ))
        }
        ast::ExpKind::Tuple(exps) => {
            let (iterctx, exps) = rename_exps_in_place(ctx, binds, renv, iterctx, exps)?;
            let exp = Spanned::new(Noted::new(ast::ExpKind::Tuple(exps), note), span);
            Ok((iterctx, exp))
        }
        ast::ExpKind::Case(not_exp) => {
            let args = not_exp.args().into_iter().cloned().collect::<Vec<_>>();
            let mixop = not_exp.to_mixop();
            let (iterctx, args) = rename_exps_in_place(ctx, binds, renv, iterctx, &args)?;
            let not_exp = Mixop::fill(&mixop, args)
                .expect("arguments obtained from the same mixfix must match its arity");
            let exp_from = Spanned::new(
                Noted::new(ast::ExpKind::Case(Box::new(not_exp)), note.clone()),
                span.clone(),
            );
            let typ = Spanned::new(note, span.clone());
            if is_singleton_case(ctx, &typ)? {
                Ok((iterctx, exp_from))
            } else {
                let pattern = ast::Pattern::Case(Box::new(mixop));
                Ok(rename_bind_match(ctx, renv, iterctx, pattern, exp_from))
            }
        }
        ast::ExpKind::Str(fields) => {
            let exps = fields
                .iter()
                .map(|(_, exp)| exp.clone())
                .collect::<Vec<_>>();
            let (iterctx, exps) = rename_exps_in_place(ctx, binds, renv, iterctx, &exps)?;
            let fields = fields
                .iter()
                .map(|(atom, _)| atom.clone())
                .zip(exps)
                .collect();
            let exp = Spanned::new(Noted::new(ast::ExpKind::Str(fields), note), span);
            Ok((iterctx, exp))
        }
        ast::ExpKind::Opt(Some(exp_inner)) => {
            let (iterctx, exp_inner) = rename_exp_in_place(ctx, binds, renv, iterctx, exp_inner)?;
            let exp_from = Spanned::new(
                Noted::new(ast::ExpKind::Opt(Some(Box::new(exp_inner))), note),
                span,
            );
            Ok(rename_bind_match(
                ctx,
                renv,
                iterctx,
                ast::Pattern::Opt(ast::OptPattern::Some),
                exp_from,
            ))
        }
        ast::ExpKind::Opt(None) => Ok(rename_bind_match(
            ctx,
            renv,
            iterctx,
            ast::Pattern::Opt(ast::OptPattern::None),
            exp.clone(),
        )),
        ast::ExpKind::List(exps) => {
            let (iterctx, exps) = rename_exps_in_place(ctx, binds, renv, iterctx, exps)?;
            let length = i64::try_from(exps.len()).expect("expression list length fits i64");
            let exp_from = Spanned::new(Noted::new(ast::ExpKind::List(exps), note), span.clone());
            let pattern = if length == 0 {
                ast::ListPattern::Nil
            } else {
                ast::ListPattern::Fixed(length)
            };
            Ok(rename_bind_match(
                ctx,
                renv,
                iterctx,
                ast::Pattern::List(pattern),
                exp_from,
            ))
        }
        ast::ExpKind::Cons(exp_h, exp_t) => {
            let (iterctx, exp_h) = rename_exp_in_place(ctx, binds, renv, iterctx, exp_h)?;
            let (iterctx, exp_t) = rename_exp_in_place(ctx, binds, renv, iterctx, exp_t)?;
            let exp_from = Spanned::new(
                Noted::new(ast::ExpKind::Cons(Box::new(exp_h), Box::new(exp_t)), note),
                span,
            );
            Ok(rename_bind_match(
                ctx,
                renv,
                iterctx,
                ast::Pattern::List(ast::ListPattern::Cons),
                exp_from,
            ))
        }
        ast::ExpKind::Iter(exp_inner, (iter, vars)) => {
            let mut iterations = vec![Iteration {
                iter: *iter,
                vars_bound: vars.clone(),
                vars_bind: vec![],
            }];
            iterations.extend(iterctx.as_slice().iter().cloned());
            let iterctx = IterationContext::from_iterations(iterations);
            let (iterctx, exp_inner) = rename_exp_in_place(ctx, binds, renv, iterctx, exp_inner)?;
            let Some(iteration) = iterctx.as_slice().first() else {
                return Err(AlgoError::new(AlgoErrorKind::EmptyIteration, span));
            };
            let exp = Spanned::new(
                Noted::new(
                    ast::ExpKind::Iter(
                        Box::new(exp_inner),
                        (iteration.iter, iteration.vars_bound.clone()),
                    ),
                    note,
                ),
                span,
            );
            let iterctx = IterationContext::from_iterations(iterctx.as_slice()[1..].to_vec());
            Ok((iterctx, exp))
        }
        _ => Ok((iterctx, exp.clone())),
    }
}

pub fn rename_exps(
    ctx: &mut Context,
    binds: &IdSet,
    renv: &mut RenameEnv,
    iterctx: IterationContext,
    exps: &[ast::Exp],
) -> Result<(IterationContext, Vec<ast::Exp>), AlgoError> {
    let mut ctx_next = ctx.clone();
    let mut renv_next = renv.clone();
    let result = rename_exps_in_place(&mut ctx_next, binds, &mut renv_next, iterctx, exps)?;
    *ctx = ctx_next;
    *renv = renv_next;
    Ok(result)
}

fn rename_exps_in_place(
    ctx: &mut Context,
    binds: &IdSet,
    renv: &mut RenameEnv,
    mut iterctx: IterationContext,
    exps: &[ast::Exp],
) -> Result<(IterationContext, Vec<ast::Exp>), AlgoError> {
    let mut exps_renamed = Vec::with_capacity(exps.len());
    for exp in exps {
        let depth = iterctx.as_slice().len();
        let mut renv_post = RenameEnv::new();
        let (iterctx_post, exp) =
            rename_exp_in_place(ctx, binds, &mut renv_post, iterctx.clone(), exp)?;
        let drop = iterctx_post.as_slice().len().saturating_sub(depth);
        iterctx = IterationContext::from_iterations(iterctx_post.as_slice()[drop..].to_vec());
        renv.append(renv_post);
        exps_renamed.push(exp);
    }
    Ok((iterctx, exps_renamed))
}

pub fn rename_arg(
    ctx: &mut Context,
    binds: &IdSet,
    renv: &mut RenameEnv,
    iterctx: IterationContext,
    arg: &ast::Arg,
) -> Result<(IterationContext, ast::Arg), AlgoError> {
    let mut ctx_next = ctx.clone();
    let mut renv_next = renv.clone();
    let result = rename_arg_in_place(&mut ctx_next, binds, &mut renv_next, iterctx, arg)?;
    *ctx = ctx_next;
    *renv = renv_next;
    Ok(result)
}

fn rename_arg_in_place(
    ctx: &mut Context,
    binds: &IdSet,
    renv: &mut RenameEnv,
    iterctx: IterationContext,
    arg: &ast::Arg,
) -> Result<(IterationContext, ast::Arg), AlgoError> {
    let ast::ArgKind::Exp(exp) = &arg.node else {
        return Ok((iterctx, arg.clone()));
    };
    let mut renv_post = RenameEnv::new();
    let (iterctx, exp) = rename_exp_in_place(ctx, binds, &mut renv_post, iterctx, exp)?;
    renv.append(renv_post);
    let arg = Spanned::new(ast::ArgKind::Exp(Box::new(exp)), arg.span.clone());
    Ok((iterctx, arg))
}

pub fn rename_args(
    ctx: &mut Context,
    binds: &IdSet,
    renv: &mut RenameEnv,
    iterctx: IterationContext,
    args: &[ast::Arg],
) -> Result<(IterationContext, Vec<ast::Arg>), AlgoError> {
    let mut ctx_next = ctx.clone();
    let mut renv_next = renv.clone();
    let result = rename_args_in_place(&mut ctx_next, binds, &mut renv_next, iterctx, args)?;
    *ctx = ctx_next;
    *renv = renv_next;
    Ok(result)
}

fn rename_args_in_place(
    ctx: &mut Context,
    binds: &IdSet,
    renv: &mut RenameEnv,
    mut iterctx: IterationContext,
    args: &[ast::Arg],
) -> Result<(IterationContext, Vec<ast::Arg>), AlgoError> {
    let mut args_renamed = Vec::with_capacity(args.len());
    for arg in args {
        let depth = iterctx.as_slice().len();
        let mut renv_post = RenameEnv::new();
        let (iterctx_post, arg) =
            rename_arg_in_place(ctx, binds, &mut renv_post, iterctx.clone(), arg)?;
        let drop = iterctx_post.as_slice().len().saturating_sub(depth);
        iterctx = IterationContext::from_iterations(iterctx_post.as_slice()[drop..].to_vec());
        renv.append(renv_post);
        args_renamed.push(arg);
    }
    Ok((iterctx, args_renamed))
}

fn empty_iterctx(iterctx: &IterationContext) -> IterationContext {
    IterationContext::from_iterations(
        iterctx
            .as_slice()
            .iter()
            .map(|entry| Iteration {
                iter: entry.iter,
                vars_bound: vec![],
                vars_bind: vec![],
            })
            .collect(),
    )
}

fn bool_exp(kind: ast::ExpKind, span: Span) -> ast::Exp {
    Spanned::new(Noted::new(kind, ast::TypKind::Bool), span)
}

fn if_prem(exp: ast::Exp, span: Span) -> ast::Prem {
    Spanned::new(ast::PremKind::If(ast::IfPrem { exp }), span)
}

fn generate_bound(
    ctx: &Context,
    destination: &ast::Var,
    exp_from: &ast::Exp,
    iterctx: &IterationContext,
) -> Result<Vec<ast::Prem>, AlgoError> {
    let exp_l = var::as_exp(true, destination);
    let kind = match &exp_from.node.kind {
        ast::ExpKind::Case(not_exp)
            if not_exp.arity() == 0
                && !is_singleton_case(
                    ctx,
                    &Spanned::new(exp_from.node.note.clone(), exp_from.span.clone()),
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
    let prem = if_prem(bool_exp(kind, exp_from.span.clone()), exp_from.span.clone());
    let mut iterctx = iterctx.clone();
    let venv = dimension::infer_exp(exp_from);
    iterctx.filter_bound(|var| {
        venv.get(&var.id)
            .is_some_and(|dim_source| dim_source.sub(&Dim::new(var.typ.clone(), var.iters.clone())))
    });
    iterctx.add_var_bound(
        destination.id.clone(),
        destination.typ.clone(),
        destination.iters.clone(),
    );
    Ok(vec![iterctx.iterate_prem(prem)])
}

fn generate_bind_match(
    destination: &ast::Var,
    pattern: &ast::Pattern,
    exp_from: &ast::Exp,
    iterctx: &IterationContext,
) -> Vec<ast::Prem> {
    let exp_to = var::as_exp(true, destination);
    let exp_match = bool_exp(
        ast::ExpKind::Match(Box::new(exp_to.clone()), pattern.clone()),
        exp_from.span.clone(),
    );
    let prem_match = if_prem(exp_match, exp_from.span.clone());
    let mut iterctx_match = empty_iterctx(iterctx);
    iterctx_match.add_var_bound(
        destination.id.clone(),
        destination.typ.clone(),
        destination.iters.clone(),
    );

    let prem_bind = Spanned::new(
        ast::PremKind::Let(ast::LetPrem {
            exp_l: exp_from.clone(),
            exp_r: exp_to,
        }),
        exp_from.span.clone(),
    );
    let mut iterctx_bind = empty_iterctx(iterctx);
    iterctx_bind.add_vars_bind(dimension::infer_exp(exp_from));
    iterctx_bind.add_var_bound(
        destination.id.clone(),
        destination.typ.clone(),
        destination.iters.clone(),
    );
    vec![
        iterctx_match.iterate_prem(prem_match),
        iterctx_bind.iterate_prem(prem_bind),
    ]
}

fn generate_bind_sub(
    ctx: &Context,
    destination: &ast::Var,
    typ_sub: &ast::Typ,
    exp_sub: &ast::Exp,
    exp_from: &ast::Exp,
    iterctx: &IterationContext,
) -> Result<Vec<ast::Prem>, AlgoError> {
    let exp_to = var::as_exp(true, destination);
    let typ_source = Spanned::new(exp_to.node.note.clone(), exp_to.span.clone());
    let subcheck = optimize_sub_typ(&ctx.tdenv, &typ_source, typ_sub)?;
    let exp_subcheck = bool_exp(
        ast::ExpKind::Sub(
            Box::new(exp_to.clone()),
            typ_sub.clone(),
            Box::new(subcheck),
        ),
        exp_from.span.clone(),
    );
    let prem_subcheck = if_prem(exp_subcheck, exp_from.span.clone());
    let mut iterctx_subcheck = empty_iterctx(iterctx);
    iterctx_subcheck.add_var_bound(
        destination.id.clone(),
        destination.typ.clone(),
        destination.iters.clone(),
    );

    let exp_downcast = Spanned::new(
        Noted::new(
            ast::ExpKind::DownCast(typ_sub.clone(), Box::new(exp_to)),
            typ_sub.node.clone(),
        ),
        exp_from.span.clone(),
    );
    let prem_bind = Spanned::new(
        ast::PremKind::Let(ast::LetPrem {
            exp_l: exp_sub.clone(),
            exp_r: exp_downcast,
        }),
        exp_from.span.clone(),
    );
    let mut iterctx_bind = empty_iterctx(iterctx);
    iterctx_bind.add_vars_bind(dimension::infer_exp(exp_from));
    iterctx_bind.add_var_bound(
        destination.id.clone(),
        destination.typ.clone(),
        destination.iters.clone(),
    );
    Ok(vec![
        iterctx_subcheck.iterate_prem(prem_subcheck),
        iterctx_bind.iterate_prem(prem_bind),
    ])
}

fn generate_prem(
    ctx: &Context,
    rename: &Rename,
    iterctx_prem: &IterationContext,
) -> Result<Vec<ast::Prem>, AlgoError> {
    let mut iterations = rename.iterctx.as_slice().to_vec();
    iterations.extend(iterctx_prem.as_slice().iter().cloned());
    let iterctx = IterationContext::from_iterations(iterations);
    match &rename.source {
        Source::Bound { exp_from } => generate_bound(ctx, &rename.destination, exp_from, &iterctx),
        Source::BindMatch { pattern, exp_from } => Ok(generate_bind_match(
            &rename.destination,
            pattern,
            exp_from,
            &iterctx,
        )),
        Source::BindSub {
            typ_sub,
            exp_sub,
            exp_from,
        } => generate_bind_sub(
            ctx,
            &rename.destination,
            typ_sub,
            exp_sub,
            exp_from,
            &iterctx,
        ),
    }
}

pub fn generate_prems(
    ctx: &Context,
    iterctx_prem: &IterationContext,
    renv: &RenameEnv,
) -> Result<Vec<ast::Prem>, AlgoError> {
    let mut prems = Vec::new();
    for rename in renv.iter() {
        prems.extend(generate_prem(ctx, rename, iterctx_prem)?);
    }
    Ok(prems)
}

pub fn destination_env(renv: &RenameEnv) -> VEnv {
    renv.iter()
        .map(|rename| {
            let mut iters = rename.destination.iters.clone();
            iters.extend(rename.iterctx.iters());
            (
                rename.destination.id.clone(),
                Dim::new(rename.destination.typ.clone(), iters),
            )
        })
        .collect()
}
