//! Full binding analysis from IL definitions to AL definitions

use crate::{
    lang::{
        al,
        common::{
            ds::set::IdSet,
            notation::mixop::Mixop,
            source::{Span, Spanned},
        },
        hints::input::{self, InputHint},
        il::ast,
        traits::free::Free,
        xl,
    },
    runtime::{
        sta::{Dim, VEnv},
        types::TypeDef,
    },
};

use super::{
    super::{AlgoError, AlgoErrorKind},
    antiunify,
    bind::{self, Bindings},
    collect,
    context::Context,
    dimension,
    iteration::{Iteration, IterationContext},
    multiple, partial,
    pattern::{self, PatternSet, PatternSets},
    shallow,
};

fn input_error(error: input::InputError, span: Span) -> AlgoError {
    AlgoError::new(AlgoErrorKind::InputHint(error), span)
}

fn add_venv(venv: &mut VEnv, additions: VEnv) {
    for (id, dim) in additions.iter() {
        venv.insert(id.clone(), dim.clone());
    }
}

fn update_venv_multiple(venv: &mut VEnv, renv: &multiple::RenameEnv) {
    for (id, ids_rename) in renv.iter() {
        let dim = venv
            .get(id)
            .expect("multiple rename environment came from bindings")
            .clone();
        for id_rename in ids_rename {
            venv.insert(id_rename.clone(), dim.clone());
        }
    }
}

fn analyze_exps_as_bind(
    mut ctx: Context,
    iterctx: &IterationContext,
    exps: &[ast::Exp],
) -> Result<(Context, VEnv, Vec<ast::Exp>, Vec<ast::Prem>), AlgoError> {
    let bindings = collect::collect_exps(&ctx, exps)?;
    let mut venv = bind::flatten(&bindings);

    let mut renv_multiple = multiple::RenameEnv::from_bindings(&bindings);
    let exps = multiple::rename_exps(&mut ctx, &mut renv_multiple, exps);
    update_venv_multiple(&mut venv, &renv_multiple);
    let side_conditions_multiple =
        multiple::generate_side_conditions(&bindings, iterctx, &renv_multiple);

    let mut renv_partial = partial::RenameEnv::new();
    let (_, exps) = partial::rename_exps(
        &mut ctx,
        &venv.domain(),
        &mut renv_partial,
        IterationContext::new(),
        &exps,
    )?;
    add_venv(&mut venv, partial::destination_env(&renv_partial));
    let mut prems = partial::generate_prems(&ctx, iterctx, &renv_partial)?;
    prems.extend(side_conditions_multiple);
    Ok((ctx, venv, exps, prems))
}

fn analyze_exp_as_bound(ctx: &Context, exp: &ast::Exp) -> Result<(), AlgoError> {
    let bindings = collect::collect_exp(ctx, exp)?;
    if bindings.is_empty() {
        Ok(())
    } else {
        Err(AlgoError::new(
            AlgoErrorKind::FreeBindings,
            exp.span.clone(),
        ))
    }
}

fn analyze_exps_as_bound(ctx: &Context, exps: &[ast::Exp]) -> Result<(), AlgoError> {
    for exp in exps {
        analyze_exp_as_bound(ctx, exp)?;
    }
    Ok(())
}

fn analyze_args_as_bind(
    mut ctx: Context,
    args: &[ast::Arg],
) -> Result<(Context, VEnv, Vec<ast::Arg>, Vec<ast::Prem>), AlgoError> {
    let bindings = collect::collect_args(&ctx, args)?;
    let mut venv = bind::flatten(&bindings);

    let mut renv_multiple = multiple::RenameEnv::from_bindings(&bindings);
    let args = multiple::rename_args(&mut ctx, &mut renv_multiple, args);
    update_venv_multiple(&mut venv, &renv_multiple);
    let side_conditions_multiple =
        multiple::generate_side_conditions(&bindings, &IterationContext::new(), &renv_multiple);

    let mut renv_partial = partial::RenameEnv::new();
    let (_, args) = partial::rename_args(
        &mut ctx,
        &venv.domain(),
        &mut renv_partial,
        IterationContext::new(),
        &args,
    )?;
    add_venv(&mut venv, partial::destination_env(&renv_partial));
    let mut prems = partial::generate_prems(&ctx, &IterationContext::new(), &renv_partial)?;
    prems.extend(side_conditions_multiple);
    Ok((ctx, venv, args, prems))
}

fn analyze_args_as_bind_shallow(
    ctx: Context,
    args: &[ast::Arg],
    span: &Span,
) -> Result<(Context, VEnv, Vec<ast::Arg>, Vec<ast::Prem>), AlgoError> {
    if !shallow::check_args(args) {
        let span = args
            .first()
            .map(|arg| arg.span.clone())
            .unwrap_or_else(|| span.clone());
        return Err(AlgoError::new(AlgoErrorKind::BindingsNotShallow, span));
    }

    let bindings = collect::collect_args(&ctx, args)?;
    let mut venv = bind::flatten(&bindings);
    let mut ctx = ctx;
    let mut renv_multiple = multiple::RenameEnv::from_bindings(&bindings);
    let args = multiple::rename_args(&mut ctx, &mut renv_multiple, args);
    update_venv_multiple(&mut venv, &renv_multiple);
    let side_conditions =
        multiple::generate_side_conditions(&bindings, &IterationContext::new(), &renv_multiple);
    if !side_conditions.is_empty() {
        return Err(AlgoError::new(
            AlgoErrorKind::ShallowSideConditions,
            args.first()
                .map(|arg| arg.span.clone())
                .unwrap_or_else(|| span.clone()),
        ));
    }

    let mut renv_partial = partial::RenameEnv::new();
    let (_, args) = partial::rename_args(
        &mut ctx,
        &venv.domain(),
        &mut renv_partial,
        IterationContext::new(),
        &args,
    )?;
    add_venv(&mut venv, partial::destination_env(&renv_partial));
    let prems = partial::generate_prems(&ctx, &IterationContext::new(), &renv_partial)?;
    Ok((ctx, venv, args, prems))
}

fn analyze_args_as_bound_shallow(ctx: &Context, args: &[ast::Arg]) -> Result<(), AlgoError> {
    for arg in args {
        if !shallow::check_arg(arg) {
            return Err(AlgoError::new(
                AlgoErrorKind::BindingsNotShallow,
                arg.span.clone(),
            ));
        }
        let bindings = collect::collect_arg(ctx, arg)?;
        if !bindings.is_empty() {
            return Err(AlgoError::new(
                AlgoErrorKind::FreeBindings,
                arg.span.clone(),
            ));
        }
    }
    Ok(())
}

fn is_pure_exp(exp: &ast::Exp) -> bool {
    match &exp.node.kind {
        ast::ExpKind::Bool(_)
        | ast::ExpKind::Num(_)
        | ast::ExpKind::Text(_)
        | ast::ExpKind::Var(_) => true,
        ast::ExpKind::Un(_, _, exp)
        | ast::ExpKind::UpCast(_, exp)
        | ast::ExpKind::DownCast(_, exp)
        | ast::ExpKind::Sub(exp, _, _)
        | ast::ExpKind::Match(exp, _)
        | ast::ExpKind::Len(exp)
        | ast::ExpKind::Dot(exp, _)
        | ast::ExpKind::Iter(exp, _) => is_pure_exp(exp),
        ast::ExpKind::Bin(_, _, exp_l, exp_r)
        | ast::ExpKind::Cmp(_, _, exp_l, exp_r)
        | ast::ExpKind::Cons(exp_l, exp_r)
        | ast::ExpKind::Cat(exp_l, exp_r)
        | ast::ExpKind::Mem(exp_l, exp_r)
        | ast::ExpKind::Idx(exp_l, exp_r) => is_pure_exp(exp_l) && is_pure_exp(exp_r),
        ast::ExpKind::Tuple(exps) | ast::ExpKind::List(exps) => exps.iter().all(is_pure_exp),
        ast::ExpKind::Case(not_exp) => not_exp.args().into_iter().all(is_pure_exp),
        ast::ExpKind::Str(fields) => fields.iter().all(|(_, exp)| is_pure_exp(exp)),
        ast::ExpKind::Opt(Some(exp)) => is_pure_exp(exp),
        ast::ExpKind::Opt(None) => true,
        ast::ExpKind::Slice(exp_b, exp_i, exp_n) => {
            is_pure_exp(exp_b) && is_pure_exp(exp_i) && is_pure_exp(exp_n)
        }
        ast::ExpKind::Upd(exp_b, path, exp_f) => {
            is_pure_exp(exp_b) && is_pure_path(path) && is_pure_exp(exp_f)
        }
        ast::ExpKind::Call(_, _, _) => false,
    }
}

fn is_pure_path(path: &ast::Path) -> bool {
    match &path.node.kind {
        ast::PathKind::Root => true,
        ast::PathKind::Idx(path, exp) => is_pure_path(path) && is_pure_exp(exp),
        ast::PathKind::Slice(path, exp_i, exp_n) => {
            is_pure_path(path) && is_pure_exp(exp_i) && is_pure_exp(exp_n)
        }
        ast::PathKind::Dot(path, _) => is_pure_path(path),
    }
}

fn is_pure_prem(prem: &ast::Prem) -> bool {
    match &prem.node {
        ast::PremKind::Rule(_)
        | ast::PremKind::If(_)
        | ast::PremKind::IfHold(_)
        | ast::PremKind::IfNotHold(_) => false,
        ast::PremKind::Let(prem) => is_pure_exp(&prem.exp_r),
        ast::PremKind::Iter(prem) => is_pure_prem(&prem.prem),
        ast::PremKind::Debug(prem) => is_pure_exp(&prem.exp),
    }
}

fn check_prems_in_else(span: &Span, prems: &[ast::Prem]) -> Result<(), AlgoError> {
    if prems.iter().all(is_pure_prem) {
        Ok(())
    } else {
        Err(AlgoError::new(
            AlgoErrorKind::ImpureElsePremises,
            span.clone(),
        ))
    }
}

fn equality_bindings(
    ctx: &Context,
    exp_l: &ast::Exp,
    exp_r: &ast::Exp,
) -> Result<(Bindings, Bindings), AlgoError> {
    Ok((
        collect::collect_exp(ctx, exp_l)?,
        collect::collect_exp(ctx, exp_r)?,
    ))
}

fn analyze_let_prem(
    mut ctx: Context,
    span: &Span,
    iterctx: IterationContext,
    exp_l: &ast::Exp,
    bindings_l: &Bindings,
    exp_r: &ast::Exp,
) -> Result<(Context, VEnv, ast::Prem, Vec<ast::Prem>), AlgoError> {
    let mut venv = bind::flatten(bindings_l);
    let mut renv_multiple = multiple::RenameEnv::from_bindings(bindings_l);
    let exp_l = multiple::rename_exp(&mut ctx, &mut renv_multiple, exp_l);
    update_venv_multiple(&mut venv, &renv_multiple);
    let side_conditions_multiple =
        multiple::generate_side_conditions(bindings_l, &iterctx, &renv_multiple);

    let mut renv_partial = partial::RenameEnv::new();
    let (_, exp_l) = partial::rename_exp(
        &mut ctx,
        &venv.domain(),
        &mut renv_partial,
        IterationContext::new(),
        &exp_l,
    )?;
    add_venv(&mut venv, partial::destination_env(&renv_partial));
    let mut prems = partial::generate_prems(&ctx, &iterctx, &renv_partial)?;
    prems.extend(side_conditions_multiple);

    let prem = Spanned::new(
        ast::PremKind::Let(ast::LetPrem {
            exp_l: exp_l.clone(),
            exp_r: exp_r.clone(),
        }),
        span.clone(),
    );
    let venv_l = dimension::infer_exp(&exp_l);
    let venv_r = dimension::infer_exp(exp_r);
    let mut iterctx = iterctx;
    iterctx.filter_bound(|var| {
        venv_r
            .get(&var.id)
            .is_some_and(|dim_r| dim_r.sub(&Dim::new(var.typ.clone(), var.iters.clone())))
    });
    iterctx.add_vars_bind(venv_l);
    iterctx.validate(span.clone())?;
    let prem = iterctx.iterate_prem(prem);
    Ok((ctx, venv, prem, prems))
}

fn analyze_prem(
    ctx: Context,
    iterctx: IterationContext,
    prem: &ast::Prem,
) -> Result<(Context, VEnv, ast::Prem, Vec<ast::Prem>), AlgoError> {
    match &prem.node {
        ast::PremKind::Rule(rule_prem) => {
            let mixop = rule_prem.not_exp.to_mixop();
            let exps = rule_prem
                .not_exp
                .args()
                .into_iter()
                .cloned()
                .collect::<Vec<_>>();
            let (exps_input, exps_output) = input::split(&rule_prem.input_hint, &exps)
                .map_err(|error| input_error(error, prem.span.clone()))?;
            analyze_exps_as_bound(&ctx, &exps_input)?;
            let (ctx, venv, exps_output, side_conditions) =
                analyze_exps_as_bind(ctx, &iterctx, &exps_output)?;
            let exps = input::combine(
                &rule_prem.input_hint,
                exps_input.clone(),
                exps_output.clone(),
            )
            .map_err(|error| input_error(error, prem.span.clone()))?;
            let not_exp = Mixop::fill(&mixop, exps)
                .expect("arguments obtained from the same mixfix must match its arity");
            let prem_analyzed = Spanned::new(
                ast::PremKind::Rule(ast::RulePrem {
                    id: rule_prem.id.clone(),
                    not_exp,
                    input_hint: rule_prem.input_hint.clone(),
                }),
                prem.span.clone(),
            );
            let venv_bound = dimension::infer_exps(&exps_input);
            let mut iterctx = iterctx;
            iterctx.filter_bound(|var| {
                venv_bound.get(&var.id).is_some_and(|dim_bound| {
                    dim_bound.sub(&Dim::new(var.typ.clone(), var.iters.clone()))
                })
            });
            iterctx.add_vars_bind(dimension::infer_exps(&exps_output));
            iterctx.validate(prem.span.clone())?;
            let prem_analyzed = iterctx.iterate_prem(prem_analyzed);
            Ok((ctx, venv, prem_analyzed, side_conditions))
        }
        ast::PremKind::If(if_prem) => {
            if let ast::ExpKind::Cmp(ast::CmpOp::Bool(xl::bool::CmpOp::Eq), _, exp_l, exp_r) =
                &if_prem.exp.node.kind
            {
                let (bindings_l, bindings_r) = equality_bindings(&ctx, exp_l, exp_r)?;
                match (bindings_l.is_empty(), bindings_r.is_empty()) {
                    (true, true) => {
                        let prem = iterctx.iterate_prem(prem.clone());
                        Ok((ctx, VEnv::new(), prem, vec![]))
                    }
                    (false, true) => {
                        analyze_let_prem(ctx, &prem.span, iterctx, exp_l, &bindings_l, exp_r)
                    }
                    (true, false) => {
                        analyze_let_prem(ctx, &prem.span, iterctx, exp_r, &bindings_r, exp_l)
                    }
                    (false, false) => Err(AlgoError::new(
                        AlgoErrorKind::BindingOnBothEqualitySides,
                        if_prem.exp.span.clone(),
                    )),
                }
            } else {
                analyze_exp_as_bound(&ctx, &if_prem.exp)?;
                let prem = iterctx.iterate_prem(prem.clone());
                Ok((ctx, VEnv::new(), prem, vec![]))
            }
        }
        ast::PremKind::IfHold(if_prem) => {
            let exps = if_prem.not_exp.args();
            for exp in exps {
                analyze_exp_as_bound(&ctx, exp)?;
            }
            let prem = iterctx.iterate_prem(prem.clone());
            Ok((ctx, VEnv::new(), prem, vec![]))
        }
        ast::PremKind::IfNotHold(if_prem) => {
            let exps = if_prem.not_exp.args();
            for exp in exps {
                analyze_exp_as_bound(&ctx, exp)?;
            }
            let prem = iterctx.iterate_prem(prem.clone());
            Ok((ctx, VEnv::new(), prem, vec![]))
        }
        ast::PremKind::Let(_) => Err(AlgoError::new(
            AlgoErrorKind::UnexpectedLetPremise,
            prem.span.clone(),
        )),
        ast::PremKind::Iter(iterated) => {
            if !iterated.iter_prem.vars_bind.is_empty() {
                return Err(AlgoError::new(
                    AlgoErrorKind::UnexpectedIterationBindings,
                    prem.span.clone(),
                ));
            }
            let mut iterations = vec![Iteration {
                iter: iterated.iter_prem.iter,
                vars_bound: iterated.iter_prem.vars_bound.clone(),
                vars_bind: vec![],
            }];
            iterations.extend(iterctx.as_slice().iter().cloned());
            analyze_prem(
                ctx,
                IterationContext::from_iterations(iterations),
                &iterated.prem,
            )
        }
        ast::PremKind::Debug(debug_prem) => {
            analyze_exp_as_bound(&ctx, &debug_prem.exp)?;
            let prem = iterctx.iterate_prem(prem.clone());
            Ok((ctx, VEnv::new(), prem, vec![]))
        }
    }
}

fn analyze_prems(
    mut ctx: Context,
    prems: &[ast::Prem],
) -> Result<(Context, Vec<ast::Prem>), AlgoError> {
    let mut prems_analyzed = Vec::new();
    for prem in prems {
        let (ctx_post, venv, prem, side_conditions) =
            analyze_prem(ctx, IterationContext::new(), prem)?;
        ctx = ctx_post;
        ctx.add_bounds(&venv);
        prems_analyzed.push(prem);
        prems_analyzed.extend(side_conditions);
    }
    Ok((ctx, prems_analyzed))
}

fn analyze_rule_match(
    ctx: &Context,
    mut ctxs_local: Vec<Context>,
    exps_input_group: Vec<Vec<ast::Exp>>,
) -> Result<(Vec<Context>, al::ast::RuleMatch, Vec<Vec<ast::Prem>>), AlgoError> {
    let mut frees = IdSet::new();
    for ctx_local in &ctxs_local {
        frees.extend(ctx_local.frees.iter().cloned());
    }
    let mut ctx_unified = ctx.clone();
    ctx_unified.add_frees(&frees);
    let (ctx_unified_post, exps_signature, prems_unified_group) =
        antiunify::antiunify(ctx_unified, exps_input_group)?;
    let (mut ctx_unified, venv, exps_input, prems) =
        analyze_exps_as_bind(ctx_unified_post, &IterationContext::new(), &exps_signature)?;
    ctx_unified.add_bounds(&venv);
    analyze_exps_as_bound(&ctx_unified, &exps_signature)?;

    for ctx_local in &mut ctxs_local {
        ctx_local.frees = ctx_unified.frees.clone();
        ctx_local.venv = ctx_unified.venv.clone();
    }
    let mut prems_analyzed_group = Vec::with_capacity(prems_unified_group.len());
    for (ctx_local, prems_unified) in ctxs_local.iter_mut().zip(prems_unified_group) {
        let (ctx_post, prems_unified) = analyze_prems(ctx_local.clone(), &prems_unified)?;
        *ctx_local = ctx_post;
        prems_analyzed_group.push(prems_unified);
    }
    let rule_match = al::ast::RuleMatch {
        exps_signature,
        exps_input,
        prems,
    };
    Ok((ctxs_local, rule_match, prems_analyzed_group))
}

fn analyze_rule_path(
    ctx: Context,
    id: ast::Id,
    prems_unified: Vec<ast::Prem>,
    prems: &[ast::Prem],
    exps_output: Vec<ast::Exp>,
    is_else: bool,
) -> Result<al::ast::RulePath, AlgoError> {
    let (ctx, prems) = analyze_prems(ctx, prems)?;
    let mut prems_all = prems_unified;
    prems_all.extend(prems);
    if is_else {
        check_prems_in_else(&id.span, &prems_all)?;
    }
    analyze_exps_as_bound(&ctx, &exps_output)?;
    Ok(al::ast::RulePath {
        id,
        prems: prems_all,
        exps_output,
    })
}

fn analyze_rule_group(
    ctx: &Context,
    inputs: &InputHint,
    rule_group: &ast::RuleGroup,
    is_else: bool,
) -> Result<al::ast::RuleGroup, AlgoError> {
    let span = rule_group.span.clone();
    let (id_group, rules) = &rule_group.node;
    let mut ctxs_local = Vec::with_capacity(rules.len());
    let mut ids = Vec::with_capacity(rules.len());
    let mut prems_group = Vec::with_capacity(rules.len());
    let mut exps_input_group = Vec::with_capacity(rules.len());
    let mut exps_output_group = Vec::with_capacity(rules.len());
    for rule in rules {
        let mut ctx_local = ctx.clone();
        ctx_local.add_frees(&rule.free());
        ctxs_local.push(ctx_local);
        ids.push(rule.node.id.clone());
        prems_group.push(rule.node.prems.clone());
        let exps = rule
            .node
            .not_exp
            .args()
            .into_iter()
            .cloned()
            .collect::<Vec<_>>();
        let (exps_input, exps_output) =
            input::split(inputs, &exps).map_err(|error| input_error(error, rule.span.clone()))?;
        exps_input_group.push(exps_input);
        exps_output_group.push(exps_output);
    }

    let (ctxs_local, rule_match, prems_unified_group) =
        analyze_rule_match(ctx, ctxs_local, exps_input_group)?;
    let mut rule_paths = Vec::with_capacity(rules.len());
    for ((((ctx_local, id), prems_unified), prems), exps_output) in ctxs_local
        .into_iter()
        .zip(ids)
        .zip(prems_unified_group)
        .zip(prems_group)
        .zip(exps_output_group)
    {
        rule_paths.push(analyze_rule_path(
            ctx_local,
            id,
            prems_unified,
            &prems,
            exps_output,
            is_else,
        )?);
    }
    let rule_group = al::ast::RuleGroupKind {
        id: id_group.clone(),
        rule_match,
        rule_paths,
    };
    Ok(Spanned::new(rule_group, span))
}

fn analyze_else_group(
    ctx: &Context,
    inputs: &InputHint,
    else_group: &ast::ElseGroup,
) -> Result<al::ast::ElseGroup, AlgoError> {
    let span = else_group.span.clone();
    let (id_group, rule) = &else_group.node;
    let rule_group = Spanned::new(
        (id_group.clone(), vec![rule.clone()]),
        else_group.span.clone(),
    );
    let rule_group = analyze_rule_group(ctx, inputs, &rule_group, true)?;
    let rule_path = rule_group
        .node
        .rule_paths
        .into_iter()
        .next()
        .expect("else groups contain one rule");
    let else_group = al::ast::ElseGroupKind {
        id: rule_group.node.id,
        rule_match: rule_group.node.rule_match,
        rule_path,
    };
    Ok(Spanned::new(else_group, span))
}

fn analyze_clause(
    ctx: &Context,
    clause: &ast::Clause,
    is_else: bool,
) -> Result<al::ast::Clause, AlgoError> {
    let mut ctx = ctx.clone();
    ctx.add_frees(&clause.free());
    let (mut ctx, venv, args, side_conditions) = analyze_args_as_bind(ctx, &clause.node.args)?;
    ctx.add_bounds(&venv);
    let (ctx, prems) = analyze_prems(ctx, &clause.node.premises)?;
    analyze_exp_as_bound(&ctx, &clause.node.expression)?;
    let mut prems_all = side_conditions;
    prems_all.extend(prems);
    if is_else {
        check_prems_in_else(&clause.span, &prems_all)?;
    }
    Ok(Spanned::new(
        ast::ClauseKind {
            args,
            expression: clause.node.expression.clone(),
            premises: prems_all,
        },
        clause.span.clone(),
    ))
}

fn pattern_set_covered_by_typ(ctx: &Context, typ: &ast::Typ) -> Result<PatternSet, AlgoError> {
    let ast::TypKind::Var(id, _) = &typ.node else {
        return Err(AlgoError::new(
            AlgoErrorKind::NonVariantPatternType,
            typ.span.clone(),
        ));
    };
    let TypeDef::Defined(_, def_typ) = ctx.find_typdef(id)? else {
        return Err(AlgoError::new(
            AlgoErrorKind::NonVariantPatternType,
            typ.span.clone(),
        ));
    };
    let ast::DefTypKind::Variant(cases) = &def_typ.node else {
        return Err(AlgoError::new(
            AlgoErrorKind::NonVariantPatternType,
            typ.span.clone(),
        ));
    };
    Ok(cases
        .iter()
        .map(|(not_typ, _, _)| not_typ.clone())
        .collect())
}

fn pattern_set_covered_by_exp(ctx: &Context, exp: &ast::Exp) -> Result<PatternSet, AlgoError> {
    match &exp.node.kind {
        ast::ExpKind::Var(_) => {
            pattern_set_covered_by_typ(ctx, &Spanned::new(exp.node.note.clone(), exp.span.clone()))
        }
        ast::ExpKind::UpCast(_, exp_inner)
            if matches!(exp_inner.node.kind, ast::ExpKind::Var(_)) =>
        {
            pattern_set_covered_by_typ(
                ctx,
                &Spanned::new(exp_inner.node.note.clone(), exp_inner.span.clone()),
            )
        }
        ast::ExpKind::UpCast(_, exp_inner) => {
            let ast::ExpKind::Case(not_exp) = &exp_inner.node.kind else {
                return Err(AlgoError::new(
                    AlgoErrorKind::InvalidTablePattern,
                    exp.span.clone(),
                ));
            };
            let not_typ = not_exp.map(|exp| Spanned::new(exp.node.note.clone(), exp.span.clone()));
            Ok([Spanned::new(not_typ, exp_inner.span.clone())]
                .into_iter()
                .collect())
        }
        _ => Err(AlgoError::new(
            AlgoErrorKind::InvalidTablePattern,
            exp.span.clone(),
        )),
    }
}

fn check_valid_table_rows(
    ctx: &Context,
    span: &Span,
    typs_match: &[ast::Typ],
    rows: &[al::ast::TableRow],
) -> Result<(), AlgoError> {
    let has_closer = rows.last().is_some_and(|row| {
        row.node
            .exps_signature
            .iter()
            .all(|exp| matches!(&exp.node.kind, ast::ExpKind::Var(id) if id.node.starts_with('_')))
    });
    let pattern_rows = if has_closer {
        &rows[..rows.len() - 1]
    } else {
        rows
    };
    let mut pattern_group = Vec::with_capacity(pattern_rows.len());
    for row in pattern_rows {
        let mut patterns = Vec::with_capacity(row.node.exps_signature.len());
        for exp in &row.node.exps_signature {
            patterns.push(pattern_set_covered_by_exp(ctx, exp)?);
        }
        pattern_group.push(patterns);
    }
    if pattern::find_overlap(span, &pattern_group)?.is_some() {
        return Err(AlgoError::new(
            AlgoErrorKind::OverlappingTablePatterns,
            span.clone(),
        ));
    }
    let mut patterns_total: PatternSets = Vec::with_capacity(typs_match.len());
    for typ in typs_match {
        patterns_total.push(pattern_set_covered_by_typ(ctx, typ)?);
    }
    let missing = pattern::find_missing(span, &patterns_total, &pattern_group)?;
    if !has_closer && !missing.is_empty() {
        return Err(AlgoError::new(
            AlgoErrorKind::MissingTablePatterns,
            span.clone(),
        ));
    }
    Ok(())
}

fn analyze_table_row(ctx: &Context, row: &ast::TableRow) -> Result<al::ast::TableRow, AlgoError> {
    let (args, exp) = &row.node;
    let mut ctx = ctx.clone();
    ctx.add_frees(&row.free());
    let (mut ctx, venv, args_input, prems) = analyze_args_as_bind_shallow(ctx, args, &row.span)?;
    ctx.add_bounds(&venv);
    analyze_args_as_bound_shallow(&ctx, args)?;
    let mut exps_signature = Vec::with_capacity(args.len());
    for arg in args {
        let ast::ArgKind::Exp(exp) = &arg.node else {
            return Err(AlgoError::new(
                AlgoErrorKind::InvalidTablePattern,
                arg.span.clone(),
            ));
        };
        exps_signature.push((**exp).clone());
    }
    analyze_exp_as_bound(&ctx, exp)?;
    Ok(Spanned::new(
        al::ast::TableRowKind {
            exps_signature,
            args: args_input,
            exp: exp.clone(),
            prems,
        },
        row.span.clone(),
    ))
}

fn analyze_table_rows(
    ctx: &Context,
    span: &Span,
    params: &[ast::Param],
    rows: &[ast::TableRow],
) -> Result<Vec<al::ast::TableRow>, AlgoError> {
    let mut rows_analyzed = Vec::with_capacity(rows.len());
    for row in rows {
        rows_analyzed.push(analyze_table_row(ctx, row)?);
    }
    let mut typs_match = Vec::with_capacity(params.len());
    for param in params {
        let ast::ParamKind::Exp(typ) = &param.node else {
            return Err(AlgoError::new(
                AlgoErrorKind::InvalidTableParameter,
                param.span.clone(),
            ));
        };
        typs_match.push(typ.clone());
    }
    check_valid_table_rows(ctx, span, &typs_match, &rows_analyzed)?;
    Ok(rows_analyzed)
}

fn analyze_def(ctx: &Context, def: &ast::Def) -> Result<al::ast::Def, AlgoError> {
    let kind = match &def.node {
        ast::DefKind::ExternTyp(def) => al::ast::DefKind::ExternTyp(al::ast::ExternTypDef {
            id: def.id.clone(),
            hints: def.hints.clone(),
        }),
        ast::DefKind::Typ(def) => al::ast::DefKind::Typ(al::ast::TypDef {
            id: def.id.clone(),
            tparams: def.tparams.clone(),
            def_typ: def.def_typ.clone(),
            hints: def.hints.clone(),
        }),
        ast::DefKind::Var(def) => al::ast::DefKind::Var(al::ast::VarDef {
            id: def.id.clone(),
            typ: def.typ.clone(),
            hints: def.hints.clone(),
        }),
        ast::DefKind::ExternRel(def) => al::ast::DefKind::ExternRel(al::ast::ExternRelDef {
            id: def.id.clone(),
            not_typ: def.not_typ.clone(),
            input_hint: def.input_hint.clone(),
            hints: def.hints.clone(),
        }),
        ast::DefKind::Rel(def) => {
            let mut rule_groups = Vec::with_capacity(def.rule_groups.len());
            for rule_group in &def.rule_groups {
                let mut analyzed = analyze_rule_group(ctx, &def.input_hint, rule_group, false)?;
                analyzed.span = rule_group.span.clone();
                rule_groups.push(analyzed);
            }
            let else_group = def
                .else_group
                .as_ref()
                .map(|group| analyze_else_group(ctx, &def.input_hint, group))
                .transpose()?;
            al::ast::DefKind::Rel(al::ast::RelDef {
                id: def.id.clone(),
                not_typ: def.not_typ.clone(),
                input_hint: def.input_hint.clone(),
                rule_groups,
                else_group,
                hints: def.hints.clone(),
            })
        }
        ast::DefKind::ExternDec(def) => al::ast::DefKind::ExternDec(al::ast::ExternDecDef {
            id: def.id.clone(),
            tparams: def.tparams.clone(),
            params: def.params.clone(),
            typ: def.typ.clone(),
            hints: def.hints.clone(),
        }),
        ast::DefKind::BuiltinDec(def) => al::ast::DefKind::BuiltinDec(al::ast::BuiltinDecDef {
            id: def.id.clone(),
            tparams: def.tparams.clone(),
            params: def.params.clone(),
            typ: def.typ.clone(),
            hints: def.hints.clone(),
        }),
        ast::DefKind::TableDec(table_def) => {
            let table_rows =
                analyze_table_rows(ctx, &def.span, &table_def.params, &table_def.rows)?;
            al::ast::DefKind::TableDec(al::ast::TableDecDef {
                id: table_def.id.clone(),
                params: table_def.params.clone(),
                typ: table_def.typ.clone(),
                table_rows,
                hints: table_def.hints.clone(),
            })
        }
        ast::DefKind::FuncDec(def) => {
            let mut clauses = Vec::with_capacity(def.clauses.len());
            for clause in &def.clauses {
                clauses.push(analyze_clause(ctx, clause, false)?);
            }
            let else_clause = def
                .else_clause
                .as_ref()
                .map(|clause| analyze_clause(ctx, clause, true))
                .transpose()?;
            al::ast::DefKind::FuncDec(al::ast::FuncDecDef {
                id: def.id.clone(),
                tparams: def.tparams.clone(),
                params: def.params.clone(),
                typ: def.typ.clone(),
                clauses,
                else_clause,
                hints: def.hints.clone(),
            })
        }
    };
    Ok(Spanned::new(kind, def.span.clone()))
}

/// Analyzes a complete IL specification before side-condition guard insertion
pub(in crate::pass::algo) fn analyze_spec(spec: &ast::Spec) -> Result<al::ast::Spec, AlgoError> {
    let mut ctx = Context::new();
    ctx.load_spec(spec);
    spec.iter().map(|def| analyze_def(&ctx, def)).collect()
}
