//! Binding analysis from IL to AL:
//!
//! 1. Collect all binding occurrences of variables in an IL construct
//!    - Check that all binding occurrences reside in invertible constructs
//! 2. Rename multi/parallel binding occurrences
//!
//!    -- let (int, int) = ...
//!
//!    becomes
//!
//!    -- let (int, int') = ..., -- if int = int'
//!
//! 3. Desugar partial bindings, occurring as either:
//!    1. Bound values occurring inside binder patterns
//!
//!       -- let PATTERN (a, 1 + 2) = ...
//!
//!       becomes
//!
//!       -- let PATTERN (a, int) = ..., -- if int == 1 + 2
//!
//!    2. Injection of a variant case
//!
//!       -- let PATTERN (a, int) = pat
//!
//!       becomes
//!
//!       -- if pat matches PATTERN, -- let PATTERN (a, b) = pat
//!
//!    3. Injection of a subtype case
//!
//!       -- let ((typ) child) = parent
//!
//!       becomes
//!
//!       -- if parent <: child, -- let child = parent as child
//!
//! At this point, binder patterns are one of:
//!
//! - `VarE`, `TupleE`, `CaseE` of a singleton case, or `StrE`
//! - `IterE` of the above cases

use crate::{
    lang::{
        al,
        common::{notation::mixop::Mixop, source::Span},
        hints::input::{self, InputHint},
        il::ast,
        traits::free::Free,
        xl,
    },
    phrase,
    runtime::{
        sta::{Dim, VEnv},
        types::TypeDef,
    },
};

use super::{
    super::{AlgoError, AlgoErrorKind},
    antiunify,
    bind::BEnv,
    collect,
    context::Context,
    dimension,
    iteration::{ICtx, Iteration},
    multiple, partial,
    pattern::{self, PatternSet},
    shallow,
};

// == Helpers

// - Errors

fn input_error(error: input::InputError, span: Span) -> AlgoError {
    AlgoError::new(AlgoErrorKind::InputHint(error), span)
}

// - Environments

fn update_venv_multiple(venv: &mut VEnv, renv: &multiple::RenameEnv) {
    for (id, ids_rename) in renv.iter() {
        let dim = venv
            .get(id)
            .expect("multiple-bound variable must exist in the variable environment")
            .clone();
        for id_rename in ids_rename {
            venv.insert(id_rename.clone(), dim.clone());
        }
    }
}

fn update_venv_partial(venv: &mut VEnv, renv: &partial::RenameEnv) {
    for rename in &renv.renames {
        let mut iters = rename.destination.iters.clone();
        iters.extend(rename.iter_ctx.iters());
        venv.insert(
            rename.destination.id.clone(),
            Dim::new(rename.destination.typ.clone(), iters),
        );
    }
}

// == Expression binding analysis

fn analyze_exps_as_bind(
    ctx: &mut Context,
    iter_ctx: &ICtx,
    exps_il: &[ast::Exp],
) -> Result<(VEnv, Vec<ast::Exp>, Vec<al::ast::Prem>), AlgoError> {
    let benv = collect::collect_exps(ctx, exps_il)?;
    let mut venv = benv.flatten();

    let mut renv_multiple = multiple::RenameEnv::from_bindings(&benv);
    let exps_al = multiple::rename_exps(ctx, &mut renv_multiple, exps_il);
    update_venv_multiple(&mut venv, &renv_multiple);
    let prem_sideconditions_multiple_al =
        multiple::generate_side_conditions(iter_ctx, &renv_multiple);

    let mut renv_partial = partial::RenameEnv::new();
    let mut iter_ctx_exp = ICtx::new();
    let exps_al = partial::rename_exps(
        ctx,
        &venv.domain(),
        &mut renv_partial,
        &mut iter_ctx_exp,
        &exps_al,
    )?;
    update_venv_partial(&mut venv, &renv_partial);
    let mut prems_al = partial::gen_prems(ctx, iter_ctx, &renv_partial)?;
    prems_al.extend(prem_sideconditions_multiple_al);
    Ok((venv, exps_al, prems_al))
}

fn analyze_exp_as_bound(ctx: &Context, exp: &ast::Exp) -> Result<(), AlgoError> {
    let benv = collect::collect_exp(ctx, exp)?;
    if benv.is_empty() {
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

// == Argument binding analysis

fn analyze_args_as_bind(
    ctx: &mut Context,
    args_il: &[ast::Arg],
) -> Result<(VEnv, Vec<ast::Arg>, Vec<al::ast::Prem>), AlgoError> {
    let benv = collect::collect_args(ctx, args_il)?;
    let mut venv = benv.flatten();

    let mut renv_multiple = multiple::RenameEnv::from_bindings(&benv);
    let args_al = multiple::rename_args(ctx, &mut renv_multiple, args_il);
    update_venv_multiple(&mut venv, &renv_multiple);
    let prem_sideconditions_multiple_al =
        multiple::generate_side_conditions(&ICtx::new(), &renv_multiple);

    let mut renv_partial = partial::RenameEnv::new();
    let mut iter_ctx_arg = ICtx::new();
    let args_al = partial::rename_args(
        ctx,
        &venv.domain(),
        &mut renv_partial,
        &mut iter_ctx_arg,
        &args_al,
    )?;
    update_venv_partial(&mut venv, &renv_partial);
    let mut prems_al = partial::gen_prems(ctx, &ICtx::new(), &renv_partial)?;
    prems_al.extend(prem_sideconditions_multiple_al);
    Ok((venv, args_al, prems_al))
}

fn analyze_args_as_bind_shallow(
    ctx: &mut Context,
    args_il: &[ast::Arg],
    span: &Span,
) -> Result<(VEnv, Vec<ast::Arg>, Vec<al::ast::Prem>), AlgoError> {
    if !shallow::check_args(args_il) {
        let span = args_il
            .first()
            .map(|arg| arg.span.clone())
            .unwrap_or_else(|| span.clone());
        return Err(AlgoError::new(AlgoErrorKind::BindingsNotShallow, span));
    }

    let benv = collect::collect_args(ctx, args_il)?;
    let mut venv = benv.flatten();
    let mut renv_multiple = multiple::RenameEnv::from_bindings(&benv);
    let args_al = multiple::rename_args(ctx, &mut renv_multiple, args_il);
    update_venv_multiple(&mut venv, &renv_multiple);
    let prem_sideconditions_al = multiple::generate_side_conditions(&ICtx::new(), &renv_multiple);
    if !prem_sideconditions_al.is_empty() {
        return Err(AlgoError::new(
            AlgoErrorKind::ShallowSideConditions,
            args_al
                .first()
                .map(|arg| arg.span.clone())
                .unwrap_or_else(|| span.clone()),
        ));
    }

    let mut renv_partial = partial::RenameEnv::new();
    let mut iter_ctx_arg = ICtx::new();
    let args_al = partial::rename_args(
        ctx,
        &venv.domain(),
        &mut renv_partial,
        &mut iter_ctx_arg,
        &args_al,
    )?;
    update_venv_partial(&mut venv, &renv_partial);
    let prems_al = partial::gen_prems(ctx, &ICtx::new(), &renv_partial)?;
    Ok((venv, args_al, prems_al))
}

fn analyze_args_as_bound_shallow(ctx: &Context, args: &[ast::Arg]) -> Result<(), AlgoError> {
    for arg in args {
        if !shallow::check_arg(arg) {
            return Err(AlgoError::new(
                AlgoErrorKind::BindingsNotShallow,
                arg.span.clone(),
            ));
        }
        let benv = collect::collect_arg(ctx, arg)?;
        if !benv.is_empty() {
            return Err(AlgoError::new(
                AlgoErrorKind::FreeBindings,
                arg.span.clone(),
            ));
        }
    }
    Ok(())
}

// == Premise binding analysis

// - Helpers

fn check_prems_in_else(span: &Span, prems: &[al::ast::Prem]) -> Result<(), AlgoError> {
    if prems.iter().all(|prem| !al::partial::is_partial_prem(prem)) {
        Ok(())
    } else {
        Err(AlgoError::new(
            AlgoErrorKind::ImpureElsePremises,
            span.clone(),
        ))
    }
}

// - Premise dispatch

fn analyze_prem(
    ctx: &mut Context,
    iter_ctx: ICtx,
    prem_il: &ast::Prem,
) -> Result<(VEnv, al::ast::Prem, Vec<al::ast::Prem>), AlgoError> {
    match &prem_il.node {
        ast::PremKind::Rule(rule_prem_il) => {
            analyze_rule_prem(ctx, iter_ctx, &prem_il.span, rule_prem_il)
        }
        ast::PremKind::If(if_prem_il) => analyze_if_prem(ctx, iter_ctx, &prem_il.span, if_prem_il),
        ast::PremKind::IfHold(if_prem_il) => {
            analyze_if_hold_prem(ctx, iter_ctx, &prem_il.span, if_prem_il)
        }
        ast::PremKind::IfNotHold(if_prem_il) => {
            analyze_if_not_hold_prem(ctx, iter_ctx, &prem_il.span, if_prem_il)
        }
        ast::PremKind::Iter(iter_prem_il) => {
            analyze_iter_prem(ctx, iter_ctx, &prem_il.span, iter_prem_il)
        }
        ast::PremKind::Debug(debug_prem_il) => {
            analyze_debug_prem(ctx, iter_ctx, &prem_il.span, debug_prem_il)
        }
    }
}

// - Rule premises

fn analyze_rule_prem(
    ctx: &mut Context,
    iter_ctx: ICtx,
    span: &Span,
    rule_prem_il: &ast::RulePrem,
) -> Result<(VEnv, al::ast::Prem, Vec<al::ast::Prem>), AlgoError> {
    let mixop = rule_prem_il.not_exp.to_mixop();
    let exps_il = rule_prem_il
        .not_exp
        .args()
        .into_iter()
        .cloned()
        .collect::<Vec<_>>();
    let (exps_input_il, exps_output_il) = input::split(&rule_prem_il.input_hint, &exps_il)
        .map_err(|error| input_error(error, span.clone()))?;
    analyze_exps_as_bound(ctx, &exps_input_il)?;
    let (venv, exps_output_al, prem_sideconditions_al) =
        analyze_exps_as_bind(ctx, &iter_ctx, &exps_output_il)?;
    let exps_al = input::combine(
        &rule_prem_il.input_hint,
        exps_input_il.clone(),
        exps_output_al.clone(),
    )
    .map_err(|error| input_error(error, span.clone()))?;
    let not_exp_al = Mixop::fill(&mixop, exps_al)
        .expect("arguments obtained from the same mixfix must match its arity");
    let prem_al = phrase! {
        node: al::ast::PremKind::Rule(al::ast::RulePrem {
            id: rule_prem_il.id.clone(),
            not_exp: not_exp_al,
            input_hint: rule_prem_il.input_hint.clone(),
        }),
        span: span.clone(),
    };
    let venv_bound = dimension::infer_exps(&exps_input_il);
    let mut iter_ctx = iter_ctx;
    iter_ctx.filter_bound(|var| {
        venv_bound
            .get(&var.id)
            .is_some_and(|dim_bound| dim_bound.sub(&Dim::new(var.typ.clone(), var.iters.clone())))
    });
    iter_ctx.add_vars_bind(dimension::infer_exps(&exps_output_al));
    iter_ctx.validate(span.clone())?;
    let prem_al = iter_ctx.iterate_prem(prem_al);
    Ok((venv, prem_al, prem_sideconditions_al))
}

// - Conditional premises

fn analyze_if_eq_prem(
    ctx: &mut Context,
    iter_ctx: ICtx,
    span: &Span,
    if_prem_il: &ast::IfPrem,
    exp_l_il: &ast::Exp,
    exp_r_il: &ast::Exp,
) -> Result<(VEnv, al::ast::Prem, Vec<al::ast::Prem>), AlgoError> {
    let benv_l = collect::collect_exp(ctx, exp_l_il)?;
    let benv_r = collect::collect_exp(ctx, exp_r_il)?;
    match (benv_l.is_empty(), benv_r.is_empty()) {
        (true, true) => {
            let prem_al = phrase! {
                node: al::ast::PremKind::If(al::ast::IfPrem {
                    exp: if_prem_il.exp.clone(),
                }),
                span: span.clone(),
            };
            Ok((VEnv::new(), iter_ctx.iterate_prem(prem_al), vec![]))
        }
        (false, true) => analyze_let_prem(ctx, span, iter_ctx, exp_l_il, &benv_l, exp_r_il),
        (true, false) => analyze_let_prem(ctx, span, iter_ctx, exp_r_il, &benv_r, exp_l_il),
        (false, false) => Err(AlgoError::new(
            AlgoErrorKind::BindingOnBothEqualitySides,
            if_prem_il.exp.span.clone(),
        )),
    }
}

fn analyze_if_prem(
    ctx: &mut Context,
    iter_ctx: ICtx,
    span: &Span,
    if_prem_il: &ast::IfPrem,
) -> Result<(VEnv, al::ast::Prem, Vec<al::ast::Prem>), AlgoError> {
    if let ast::ExpKind::Cmp(ast::CmpOp::Bool(xl::bool::CmpOp::Eq), _, exp_l_il, exp_r_il) =
        &if_prem_il.exp.node
    {
        analyze_if_eq_prem(ctx, iter_ctx, span, if_prem_il, exp_l_il, exp_r_il)
    } else {
        analyze_exp_as_bound(ctx, &if_prem_il.exp)?;
        let prem_al = phrase! {
            node: al::ast::PremKind::If(al::ast::IfPrem {
                exp: if_prem_il.exp.clone(),
            }),
            span: span.clone(),
        };
        Ok((VEnv::new(), iter_ctx.iterate_prem(prem_al), vec![]))
    }
}

// - Holding premises

fn analyze_if_hold_prem(
    ctx: &mut Context,
    iter_ctx: ICtx,
    span: &Span,
    if_prem_il: &ast::IfHoldPrem,
) -> Result<(VEnv, al::ast::Prem, Vec<al::ast::Prem>), AlgoError> {
    for exp_il in if_prem_il.not_exp.args() {
        analyze_exp_as_bound(ctx, exp_il)?;
    }
    let prem_al = phrase! {
        node: al::ast::PremKind::IfHold(al::ast::IfHoldPrem {
            id: if_prem_il.id.clone(),
            not_exp: if_prem_il.not_exp.clone(),
        }),
        span: span.clone(),
    };
    Ok((VEnv::new(), iter_ctx.iterate_prem(prem_al), vec![]))
}

// - Non-holding premises

fn analyze_if_not_hold_prem(
    ctx: &mut Context,
    iter_ctx: ICtx,
    span: &Span,
    if_prem_il: &ast::IfNotHoldPrem,
) -> Result<(VEnv, al::ast::Prem, Vec<al::ast::Prem>), AlgoError> {
    for exp_il in if_prem_il.not_exp.args() {
        analyze_exp_as_bound(ctx, exp_il)?;
    }
    let prem_al = phrase! {
        node: al::ast::PremKind::IfNotHold(al::ast::IfNotHoldPrem {
            id: if_prem_il.id.clone(),
            not_exp: if_prem_il.not_exp.clone(),
        }),
        span: span.clone(),
    };
    Ok((VEnv::new(), iter_ctx.iterate_prem(prem_al), vec![]))
}

// - Let premises

fn analyze_let_prem(
    ctx: &mut Context,
    span: &Span,
    iter_ctx: ICtx,
    exp_l_il: &ast::Exp,
    benv_l: &BEnv,
    exp_r_il: &ast::Exp,
) -> Result<(VEnv, al::ast::Prem, Vec<al::ast::Prem>), AlgoError> {
    let mut venv = benv_l.flatten();
    let mut renv_multiple = multiple::RenameEnv::from_bindings(benv_l);
    let exp_l_al = multiple::rename_exp(ctx, &mut renv_multiple, exp_l_il);
    update_venv_multiple(&mut venv, &renv_multiple);
    let prem_sideconditions_multiple_al =
        multiple::generate_side_conditions(&iter_ctx, &renv_multiple);

    let mut renv_partial = partial::RenameEnv::new();
    let mut iter_ctx_exp = ICtx::new();
    let exp_l_al = partial::rename_exp(
        ctx,
        &venv.domain(),
        &mut renv_partial,
        &mut iter_ctx_exp,
        &exp_l_al,
    )?;
    update_venv_partial(&mut venv, &renv_partial);
    let mut prems_al = partial::gen_prems(ctx, &iter_ctx, &renv_partial)?;
    prems_al.extend(prem_sideconditions_multiple_al);

    let prem_al = phrase! {
        node: al::ast::PremKind::Let(al::ast::LetPrem {
            exp_l: exp_l_al.clone(),
            exp_r: exp_r_il.clone(),
        }),
        span: span.clone(),
    };
    let venv_l = dimension::infer_exp(&exp_l_al);
    let venv_r = dimension::infer_exp(exp_r_il);
    let mut iter_ctx = iter_ctx;
    iter_ctx.filter_bound(|var| {
        venv_r
            .get(&var.id)
            .is_some_and(|dim_r| dim_r.sub(&Dim::new(var.typ.clone(), var.iters.clone())))
    });
    iter_ctx.add_vars_bind(venv_l);
    iter_ctx.validate(span.clone())?;
    let prem_al = iter_ctx.iterate_prem(prem_al);
    Ok((venv, prem_al, prems_al))
}

// - Iteration premises

fn analyze_iter_prem(
    ctx: &mut Context,
    iter_ctx: ICtx,
    span: &Span,
    iter_prem_il: &ast::IterPrem,
) -> Result<(VEnv, al::ast::Prem, Vec<al::ast::Prem>), AlgoError> {
    if !iter_prem_il.prem_iter.vars_bind.is_empty() {
        return Err(AlgoError::new(
            AlgoErrorKind::UnexpectedIterationBindings,
            span.clone(),
        ));
    }
    let mut iterations = vec![Iteration {
        iter: iter_prem_il.prem_iter.iter,
        vars_bound: iter_prem_il.prem_iter.vars_bound.clone(),
        vars_bind: vec![],
    }];
    iterations.extend(iter_ctx.as_slice().iter().cloned());
    analyze_prem(ctx, ICtx::from_iterations(iterations), &iter_prem_il.prem)
}

// - Debug premises

fn analyze_debug_prem(
    ctx: &mut Context,
    iter_ctx: ICtx,
    span: &Span,
    debug_prem_il: &ast::DebugPrem,
) -> Result<(VEnv, al::ast::Prem, Vec<al::ast::Prem>), AlgoError> {
    analyze_exp_as_bound(ctx, &debug_prem_il.exp)?;
    let prem_al = phrase! {
        node: al::ast::PremKind::Debug(al::ast::DebugPrem {
            exp: debug_prem_il.exp.clone(),
        }),
        span: span.clone(),
    };
    Ok((VEnv::new(), iter_ctx.iterate_prem(prem_al), vec![]))
}

// - Premise lists

fn analyze_prems(
    ctx: &mut Context,
    prems_il: &[ast::Prem],
) -> Result<Vec<al::ast::Prem>, AlgoError> {
    let mut prems_al = Vec::new();
    for prem_il in prems_il {
        let (venv, prem_al, prem_sideconditions_al) = analyze_prem(ctx, ICtx::new(), prem_il)?;
        ctx.add_bounds(&venv);
        prems_al.push(prem_al);
        prems_al.extend(prem_sideconditions_al);
    }
    Ok(prems_al)
}

// == Rule binding analysis

#[allow(clippy::type_complexity)]
fn analyze_rule_match(
    ctx: &mut Context,
    exps_input_group_il: Vec<Vec<ast::Exp>>,
) -> Result<(al::ast::RuleMatch, Vec<Vec<ast::Prem>>), AlgoError> {
    let (exps_signature_al, prems_unified_group_il) =
        antiunify::antiunify(ctx, exps_input_group_il)?;
    let (venv, exps_input_al, prems_al) =
        analyze_exps_as_bind(ctx, &ICtx::new(), &exps_signature_al)?;
    ctx.add_bounds(&venv);
    analyze_exps_as_bound(ctx, &exps_signature_al)?;

    let rule_match_al = al::ast::RuleMatch {
        exps_signature: exps_signature_al,
        exps_input: exps_input_al,
        prems: prems_al,
    };
    Ok((rule_match_al, prems_unified_group_il))
}

fn analyze_rule_path(
    ctx: &mut Context,
    id: ast::Id,
    prems_unified_al: Vec<al::ast::Prem>,
    prems_il: &[ast::Prem],
    exps_output_il: Vec<ast::Exp>,
    is_else: bool,
) -> Result<al::ast::RulePath, AlgoError> {
    let prems_al = analyze_prems(ctx, prems_il)?;
    let mut prems_all_al = prems_unified_al;
    prems_all_al.extend(prems_al);
    if is_else {
        check_prems_in_else(&id.span, &prems_all_al)?;
    }
    analyze_exps_as_bound(ctx, &exps_output_il)?;
    Ok(al::ast::RulePath {
        id,
        prems: prems_all_al,
        exps_output: exps_output_il,
    })
}

fn analyze_rule_group(
    ctx: &mut Context,
    inputs: &InputHint,
    rule_group_il: &ast::RuleGroup,
    is_else: bool,
) -> Result<al::ast::RuleGroup, AlgoError> {
    let mut ctx = ctx.scope();
    let span = rule_group_il.span.clone();
    let (id_group, rules_il) = &rule_group_il.node;
    let mut ids = Vec::with_capacity(rules_il.len());
    let mut prems_group_il = Vec::with_capacity(rules_il.len());
    let mut exps_input_group_il = Vec::with_capacity(rules_il.len());
    let mut exps_output_group_il = Vec::with_capacity(rules_il.len());
    for rule_il in rules_il {
        ctx.add_frees(&rule_il.free());
        ids.push(rule_il.node.id.clone());
        prems_group_il.push(rule_il.node.prems.clone());
        let exps_il = rule_il
            .node
            .not_exp
            .args()
            .into_iter()
            .cloned()
            .collect::<Vec<_>>();
        let (exps_input_il, exps_output_il) = input::split(inputs, &exps_il)
            .map_err(|error| input_error(error, rule_il.span.clone()))?;
        exps_input_group_il.push(exps_input_il);
        exps_output_group_il.push(exps_output_il);
    }

    let (rule_match_al, prems_unified_group_il) =
        analyze_rule_match(&mut ctx, exps_input_group_il)?;
    let mut rule_paths_al = Vec::with_capacity(rules_il.len());
    for (((id, prems_unified_il), prems_il), exps_output_il) in ids
        .into_iter()
        .zip(prems_unified_group_il)
        .zip(prems_group_il)
        .zip(exps_output_group_il)
    {
        let mut ctx_local = ctx.scope();
        let prems_unified_al = analyze_prems(&mut ctx_local, &prems_unified_il)?;
        rule_paths_al.push(analyze_rule_path(
            &mut ctx_local,
            id,
            prems_unified_al,
            &prems_il,
            exps_output_il,
            is_else,
        )?);
    }
    let rule_group_al = al::ast::RuleGroupKind {
        id: id_group.clone(),
        rule_match: rule_match_al,
        rule_paths: rule_paths_al,
    };
    let rule_group_al = phrase!(node: rule_group_al, span: span);
    Ok(rule_group_al)
}

fn analyze_else_group(
    ctx: &mut Context,
    inputs: &InputHint,
    else_group_il: &ast::ElseGroup,
) -> Result<al::ast::ElseGroup, AlgoError> {
    let span = else_group_il.span.clone();
    let (id_group, rule_il) = &else_group_il.node;
    let rule_group_il = phrase! {
        node: (id_group.clone(), vec![rule_il.clone()]),
        span: else_group_il.span.clone(),
    };
    let rule_group_al = analyze_rule_group(ctx, inputs, &rule_group_il, true)?;
    let rule_path_al = rule_group_al
        .node
        .rule_paths
        .into_iter()
        .next()
        .expect("else groups contain one rule");
    let else_group_al = al::ast::ElseGroupKind {
        id: rule_group_al.node.id,
        rule_match: rule_group_al.node.rule_match,
        rule_path: rule_path_al,
    };
    let else_group_al = phrase!(node: else_group_al, span: span);
    Ok(else_group_al)
}

// == Clause binding analysis

fn analyze_clause(
    ctx: &mut Context,
    clause_il: ast::Clause,
    is_else: bool,
) -> Result<al::ast::Clause, AlgoError> {
    let mut ctx = ctx.scope();
    ctx.add_frees(&clause_il.free());
    let span = clause_il.span;
    let ast::ClauseKind {
        args: args_il,
        expression: exp_il,
        premises: prems_il,
    } = clause_il.node;
    let (venv, args_al, prem_sideconditions_al) = analyze_args_as_bind(&mut ctx, &args_il)?;
    ctx.add_bounds(&venv);
    let prems_al = analyze_prems(&mut ctx, &prems_il)?;
    analyze_exp_as_bound(&ctx, &exp_il)?;
    let mut prems_all_al = prem_sideconditions_al;
    prems_all_al.extend(prems_al);
    if is_else {
        check_prems_in_else(&span, &prems_all_al)?;
    }
    let clause_al = phrase! {
        node: al::ast::ClauseKind {
            args: args_al,
            expression: exp_il,
            premises: prems_all_al,
        },
        span: span,
    };
    Ok(clause_al)
}

// == Table row binding analysis

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
    let pattern_set = cases
        .iter()
        .map(|(not_typ, _, _)| not_typ.clone())
        .collect();
    Ok(pattern_set)
}

fn pattern_set_covered_by_exp(ctx: &Context, exp_al: &ast::Exp) -> Result<PatternSet, AlgoError> {
    match &exp_al.node {
        ast::ExpKind::Var(_) => {
            let typ = phrase!(node: exp_al.note.clone(), span: exp_al.span.clone());
            pattern_set_covered_by_typ(ctx, &typ)
        }
        ast::ExpKind::UpCast(_, exp_inner) if matches!(exp_inner.node, ast::ExpKind::Var(_)) => {
            let typ = phrase!(node: exp_inner.note.clone(), span: exp_inner.span.clone());
            pattern_set_covered_by_typ(ctx, &typ)
        }
        ast::ExpKind::UpCast(_, exp_inner) => {
            let ast::ExpKind::Case(not_exp) = &exp_inner.node else {
                return Err(AlgoError::new(
                    AlgoErrorKind::InvalidTablePattern,
                    exp_al.span.clone(),
                ));
            };
            let not_typ =
                not_exp.map(|exp| phrase!(node: exp.note.clone(), span: exp.span.clone()));
            let not_typ = phrase!(node: not_typ, span: exp_inner.span.clone());
            let pattern_set = [not_typ].into_iter().collect();
            Ok(pattern_set)
        }
        _ => Err(AlgoError::new(
            AlgoErrorKind::InvalidTablePattern,
            exp_al.span.clone(),
        )),
    }
}

fn check_valid_table_rows(
    ctx: &Context,
    span: &Span,
    typs_match_il: &[ast::Typ],
    rows_al: &[al::ast::TableRow],
) -> Result<(), AlgoError> {
    let has_closer =
        if let Some(row_al) = rows_al.last() {
            row_al.node.exps_signature.iter().all(
                |exp_al| matches!(&exp_al.node, ast::ExpKind::Var(id) if id.node.starts_with('_')),
            )
        } else {
            false
        };
    let rows_pattern_al = if has_closer {
        &rows_al[..rows_al.len() - 1]
    } else {
        rows_al
    };
    let mut pattern_sets_group = Vec::with_capacity(rows_pattern_al.len());
    for row_al in rows_pattern_al {
        let mut pattern_sets = Vec::with_capacity(row_al.node.exps_signature.len());
        for exp_al in &row_al.node.exps_signature {
            let pattern_set = pattern_set_covered_by_exp(ctx, exp_al)?;
            pattern_sets.push(pattern_set);
        }
        let pattern_sets = pattern_sets.into_iter().collect();
        pattern_sets_group.push(pattern_sets);
    }
    let pattern_sets_overlap = pattern::find_overlap(span, &pattern_sets_group)?;
    if pattern_sets_overlap.is_some() {
        return Err(AlgoError::new(
            AlgoErrorKind::OverlappingTablePatterns,
            span.clone(),
        ));
    }
    let mut pattern_sets_total = Vec::with_capacity(typs_match_il.len());
    for typ_il in typs_match_il {
        let pattern_set = pattern_set_covered_by_typ(ctx, typ_il)?;
        pattern_sets_total.push(pattern_set);
    }
    let pattern_sets_total = pattern_sets_total.into_iter().collect();
    let pattern_sets_group_missing =
        pattern::find_missing(span, &pattern_sets_total, &pattern_sets_group)?;
    if !has_closer && !pattern_sets_group_missing.is_empty() {
        return Err(AlgoError::new(
            AlgoErrorKind::MissingTablePatterns,
            span.clone(),
        ));
    }
    Ok(())
}

fn analyze_table_row(
    ctx: &mut Context,
    row_il: ast::TableRow,
) -> Result<al::ast::TableRow, AlgoError> {
    let mut ctx = ctx.scope();
    ctx.add_frees(&row_il.free());
    let span = row_il.span;
    let (args_il, exp_il) = row_il.node;
    let (venv, args_input_al, prems_al) = analyze_args_as_bind_shallow(&mut ctx, &args_il, &span)?;
    ctx.add_bounds(&venv);
    analyze_args_as_bound_shallow(&ctx, &args_il)?;
    let mut exps_signature_al = Vec::with_capacity(args_il.len());
    for arg_il in args_il {
        let ast::ArgKind::Exp(exp_il) = arg_il.node else {
            return Err(AlgoError::new(
                AlgoErrorKind::InvalidTablePattern,
                arg_il.span,
            ));
        };
        exps_signature_al.push(*exp_il);
    }
    analyze_exp_as_bound(&ctx, &exp_il)?;
    let row_al = phrase! {
        node: al::ast::TableRowKind {
            exps_signature: exps_signature_al,
            args: args_input_al,
            exp: exp_il,
            prems: prems_al,
        },
        span: span,
    };
    Ok(row_al)
}

fn analyze_table_rows(
    ctx: &mut Context,
    span: &Span,
    params_il: &[ast::Param],
    rows_il: Vec<ast::TableRow>,
) -> Result<Vec<al::ast::TableRow>, AlgoError> {
    let mut rows_al = Vec::with_capacity(rows_il.len());
    for row_il in rows_il {
        rows_al.push(analyze_table_row(ctx, row_il)?);
    }
    let mut typs_match_il = Vec::with_capacity(params_il.len());
    for param_il in params_il {
        let ast::ParamKind::Exp(typ_il) = &param_il.node else {
            return Err(AlgoError::new(
                AlgoErrorKind::InvalidTableParameter,
                param_il.span.clone(),
            ));
        };
        typs_match_il.push(typ_il.clone());
    }
    check_valid_table_rows(ctx, span, &typs_match_il, &rows_al)?;
    Ok(rows_al)
}

// == Definition binding analysis

// - Dispatch

fn analyze_def(ctx: &mut Context, def_il: ast::Def) -> Result<al::ast::Def, AlgoError> {
    let span = def_il.span;
    let def_kind_al = match def_il.node {
        ast::DefKind::ExternTyp(def_il) => {
            let def_al = analyze_extern_typ_def(def_il);
            al::ast::DefKind::ExternTyp(def_al)
        }
        ast::DefKind::Typ(def_il) => {
            let def_al = analyze_typ_def(def_il);
            al::ast::DefKind::Typ(def_al)
        }
        ast::DefKind::Var(def_il) => {
            let def_al = analyze_var_def(def_il);
            al::ast::DefKind::Var(def_al)
        }
        ast::DefKind::ExternRel(def_il) => {
            let def_al = analyze_extern_rel_def(def_il);
            al::ast::DefKind::ExternRel(def_al)
        }
        ast::DefKind::Rel(def_il) => {
            let def_al = analyze_rel_def(ctx, def_il)?;
            al::ast::DefKind::Rel(def_al)
        }
        ast::DefKind::ExternDec(def_il) => {
            let def_al = analyze_extern_dec_def(def_il);
            al::ast::DefKind::ExternDec(def_al)
        }
        ast::DefKind::BuiltinDec(def_il) => {
            let def_al = analyze_builtin_dec_def(def_il);
            al::ast::DefKind::BuiltinDec(def_al)
        }
        ast::DefKind::TableDec(table_def_il) => {
            let def_al = analyze_table_def(ctx, table_def_il, &span)?;
            al::ast::DefKind::TableDec(def_al)
        }
        ast::DefKind::FuncDec(def_il) => {
            let def_al = analyze_func_def(ctx, def_il)?;
            al::ast::DefKind::FuncDec(def_al)
        }
    };
    let def_al = phrase!(node: def_kind_al, span: span);
    Ok(def_al)
}

// - External type definitions

fn analyze_extern_typ_def(def_il: ast::ExternTyp) -> al::ast::ExternTypDef {
    al::ast::ExternTypDef {
        id: def_il.id,
        hints: def_il.hints,
    }
}

// - Type definitions

fn analyze_typ_def(def_il: ast::TypDef) -> al::ast::TypDef {
    al::ast::TypDef {
        id: def_il.id,
        tparams: def_il.tparams,
        def_typ: def_il.def_typ,
        hints: def_il.hints,
    }
}

// - Variable definitions

fn analyze_var_def(def_il: ast::VarDef) -> al::ast::VarDef {
    al::ast::VarDef {
        id: def_il.id,
        typ: def_il.typ,
        hints: def_il.hints,
    }
}

// - External relation definitions

fn analyze_extern_rel_def(def_il: ast::ExternRel) -> al::ast::ExternRelDef {
    al::ast::ExternRelDef {
        id: def_il.id,
        not_typ: def_il.not_typ,
        input_hint: def_il.input_hint,
        hints: def_il.hints,
    }
}

// - Relation definitions

fn analyze_rel_def(ctx: &mut Context, def_il: ast::Rel) -> Result<al::ast::RelDef, AlgoError> {
    let mut rule_groups_al = Vec::with_capacity(def_il.rule_groups.len());
    for rule_group_il in &def_il.rule_groups {
        let mut rule_group_al = analyze_rule_group(ctx, &def_il.input_hint, rule_group_il, false)?;
        rule_group_al.span = rule_group_il.span.clone();
        rule_groups_al.push(rule_group_al);
    }
    let else_group_al = def_il
        .else_group
        .as_ref()
        .map(|else_group_il| analyze_else_group(ctx, &def_il.input_hint, else_group_il))
        .transpose()?;
    Ok(al::ast::RelDef {
        id: def_il.id,
        not_typ: def_il.not_typ,
        input_hint: def_il.input_hint,
        rule_groups: rule_groups_al,
        else_group: else_group_al,
        hints: def_il.hints,
    })
}

// - External declaration definitions

fn analyze_extern_dec_def(def_il: ast::ExternDec) -> al::ast::ExternDecDef {
    al::ast::ExternDecDef {
        id: def_il.id,
        tparams: def_il.tparams,
        params: def_il.params,
        typ: def_il.typ,
        hints: def_il.hints,
    }
}

// - Builtin declaration definitions

fn analyze_builtin_dec_def(def_il: ast::BuiltinDec) -> al::ast::BuiltinDecDef {
    al::ast::BuiltinDecDef {
        id: def_il.id,
        tparams: def_il.tparams,
        params: def_il.params,
        typ: def_il.typ,
        hints: def_il.hints,
    }
}

// - Table definitions

fn analyze_table_def(
    ctx: &mut Context,
    def_il: ast::TableDec,
    span: &Span,
) -> Result<al::ast::TableDecDef, AlgoError> {
    let table_rows_al = analyze_table_rows(ctx, span, &def_il.params, def_il.rows)?;
    Ok(al::ast::TableDecDef {
        id: def_il.id,
        params: def_il.params,
        typ: def_il.typ,
        table_rows: table_rows_al,
        hints: def_il.hints,
    })
}

// - Function definitions

fn analyze_func_def(
    ctx: &mut Context,
    def_il: ast::FuncDec,
) -> Result<al::ast::FuncDecDef, AlgoError> {
    let mut clauses_al = Vec::with_capacity(def_il.clauses.len());
    for clause_il in def_il.clauses {
        clauses_al.push(analyze_clause(ctx, clause_il, false)?);
    }
    let else_clause_al = def_il
        .else_clause
        .map(|clause_il| analyze_clause(ctx, clause_il, true))
        .transpose()?;
    Ok(al::ast::FuncDecDef {
        id: def_il.id,
        tparams: def_il.tparams,
        params: def_il.params,
        typ: def_il.typ,
        clauses: clauses_al,
        else_clause: else_clause_al,
        hints: def_il.hints,
    })
}

// - Specification

/// Binding analysis of an IL specification
pub(in crate::pass::algo) fn analyze_spec(spec_il: ast::Spec) -> Result<al::ast::Spec, AlgoError> {
    let mut ctx = Context::new();
    ctx.load_spec(&spec_il);
    let mut defs_al = Vec::with_capacity(spec_il.len());
    for def_il in spec_il {
        defs_al.push(analyze_def(&mut ctx, def_il)?);
    }
    Ok(defs_al)
}
