//! Recognize compact prose instructions after annotation

use std::collections::VecDeque;

use crate::lang::{il::ast::OptPattern, pl::ast as pl};

// == Expression aliases

fn eq_exp_var(exp_a: &pl::Exp, exp_b: &pl::Exp) -> bool {
    match (&exp_a.node.node, &exp_b.node.node) {
        (pl::ExpKind::Var(id_a), pl::ExpKind::Var(id_b)) => id_a.node == id_b.node,
        (pl::ExpKind::Iter(exp_a, _), pl::ExpKind::Iter(exp_b, _)) => eq_exp_var(exp_a, exp_b),
        _ => false,
    }
}

fn is_scrutinee_alias(exp_scrut: &pl::Exp, exp_r: &pl::Exp) -> bool {
    match &exp_r.node.node {
        pl::ExpKind::DownCast(_, exp_inner) => eq_exp_var(exp_scrut, exp_inner),
        _ => eq_exp_var(exp_scrut, exp_r),
    }
}

fn has_leading_rename<Tier>(exp_scrut: &pl::Exp, block: &pl::Block<Tier>) -> bool {
    matches!(
        block.first(),
        Some(instr) if matches!(
            &instr.node.node,
            pl::InstrKind::Let(pl::LetInstr { exp_r, iter_instrs, .. })
                if iter_instrs.is_empty() && is_scrutinee_alias(exp_scrut, exp_r)
        )
    )
}

fn take_leading_target<Tier>(block: &mut pl::Block<Tier>) -> pl::Exp {
    let instr = block.remove(0);
    let pl::InstrKind::Let(pl::LetInstr { exp_l, .. }) = instr.node.node else {
        unreachable!("leading rename was checked before consumption");
    };
    exp_l
}

// == Case guards

fn shorten_case_guards<Tier>(instr: &mut pl::Instr<Tier>) {
    let pl::InstrKind::Case(pl::CaseInstr { exp, cases, .. }) = &mut instr.node.node else {
        return;
    };
    for case in cases {
        if !has_leading_rename(exp, &case.block) {
            continue;
        }
        let guard = match &case.guard {
            pl::Guard::Sub(typ, subcheck) => {
                let exp_target = take_leading_target(&mut case.block);
                Some(pl::Guard::CheckLetSub(typ.clone(), subcheck.clone(), exp_target))
            }
            pl::Guard::Match(pattern) => {
                let exp_target = take_leading_target(&mut case.block);
                Some(pl::Guard::CheckLetMatch(pattern.clone(), exp_target))
            }
            _ => None,
        };
        if let Some(guard) = guard {
            case.guard = guard;
        }
    }
}

// == Checked bindings

fn shorten_check_let<Tier>(instr: &mut pl::Instr<Tier>) {
    enum Check {
        Sub(pl::Typ, Box<pl::Subcheck>, pl::Exp),
        Match(pl::Pattern, pl::Exp),
    }

    let check = match &instr.node.node {
        pl::InstrKind::If(pl::IfInstr { exp, iter_exps, block, .. }) if iter_exps.is_empty() => {
            match &exp.node.node {
                pl::ExpKind::Sub(exp_scrut, typ, subcheck)
                    if has_leading_rename(exp_scrut, block) =>
                {
                    Some(Check::Sub(typ.clone(), subcheck.clone(), exp_scrut.as_ref().clone()))
                }
                pl::ExpKind::Match(exp_scrut, pattern) if has_leading_rename(exp_scrut, block) => {
                    Some(Check::Match(pattern.clone(), exp_scrut.as_ref().clone()))
                }
                _ => None,
            }
        }
        pl::InstrKind::Case(pl::CaseInstr { exp, cases, .. }) if cases.len() == 1 => {
            let case = &cases[0];
            if !has_leading_rename(exp, &case.block) {
                return;
            }
            match &case.guard {
                pl::Guard::Sub(typ, subcheck) => {
                    Some(Check::Sub(typ.clone(), subcheck.clone(), exp.clone()))
                }
                pl::Guard::Match(pattern) => Some(Check::Match(pattern.clone(), exp.clone())),
                _ => None,
            }
        }
        _ => None,
    };
    let Some(check) = check else { return };

    let block = match &mut instr.node.node {
        pl::InstrKind::If(pl::IfInstr { block, .. }) => block,
        pl::InstrKind::Case(pl::CaseInstr { cases, .. }) => &mut cases[0].block,
        _ => unreachable!(),
    };
    let exp_l = take_leading_target(block);
    let block = std::mem::take(block);
    instr.node.node = match check {
        Check::Sub(typ, subcheck, exp_r) => {
            pl::InstrKind::CheckLetSub(pl::CheckLetSubInstr { typ, subcheck, exp_l, exp_r, block })
        }
        Check::Match(pattern, exp_r) => {
            pl::InstrKind::CheckLetMatch(pl::CheckLetMatchInstr { pattern, exp_l, exp_r, block })
        }
    };
}

// == Destructuring

fn visible(exp: &pl::Exp) -> bool {
    match &exp.node.node {
        pl::ExpKind::Var(id) => !id.node.starts_with('_'),
        pl::ExpKind::Iter(exp_inner, _) => visible(exp_inner),
        _ => true,
    }
}

fn shorten_destruct<Tier>(instr: &mut pl::Instr<Tier>) {
    let Some(field_names) = instr.hints.prose_fields.as_ref().map(|hint| hint.fields()) else {
        return;
    };
    let pl::InstrKind::Let(pl::LetInstr { exp_l, exp_r, iter_instrs }) = &instr.node.node else {
        return;
    };
    if !iter_instrs.is_empty() {
        return;
    }
    let pl::ExpKind::Case(not_exp) = &exp_l.node.node else {
        return;
    };
    let exps = not_exp.args();
    if exps.len() != field_names.len() || exps.iter().all(|exp| !visible(exp)) {
        return;
    }
    let bindings = exps
        .into_iter()
        .zip(field_names)
        .map(|(exp, name)| (visible(exp).then(|| name.clone()), exp.clone()))
        .collect();
    instr.node.node = pl::InstrKind::Destruct(pl::DestructInstr { bindings, exp: exp_r.clone() });
}

fn shorten_instr_shorthands<Tier>(mut instr: pl::Instr<Tier>) -> pl::Instr<Tier> {
    shorten_case_guards(&mut instr);
    shorten_check_let(&mut instr);
    shorten_destruct(&mut instr);
    instr
}

// == Option extraction

fn shorten_option_get<Tier>(
    instrs_pending: &mut VecDeque<pl::Instr<Tier>>,
) -> Option<pl::Instr<Tier>> {
    if instrs_pending.len() < 2 {
        return None;
    }
    let is_option_get =
        match (&instrs_pending.front()?.node.node, &instrs_pending.get(1)?.node.node) {
            (
                pl::InstrKind::Let(pl::LetInstr {
                    exp_l: exp_tmp,
                    exp_r: exp_value,
                    iter_instrs: iter_let,
                }),
                pl::InstrKind::If(pl::IfInstr {
                    exp: exp_cond,
                    iter_exps: iter_if,
                    block: block_then,
                    ..
                }),
            ) if iter_let.is_empty() && iter_if.is_empty() => {
                let pl::ExpKind::Match(exp_scrut, pl::Pattern::Opt(OptPattern::Some)) =
                    &exp_cond.node.node
                else {
                    return None;
                };
                let instr_then = block_then.first()?;
                let pl::InstrKind::Let(pl::LetInstr { exp_l, exp_r, iter_instrs }) =
                    &instr_then.node.node
                else {
                    return None;
                };
                let pl::ExpKind::Opt(Some(exp_target)) = &exp_l.node.node else {
                    return None;
                };
                if !iter_instrs.is_empty()
                    || !eq_exp_var(exp_tmp, exp_scrut)
                    || !eq_exp_var(exp_tmp, exp_r)
                {
                    return None;
                }
                let _ = (exp_target, exp_value);
                true
            }
            _ => false,
        };
    if !is_option_get {
        return None;
    }
    let instr_source = instrs_pending.pop_front()?;
    let instr_if = instrs_pending.pop_front()?;
    let pl::InstrKind::Let(pl::LetInstr { exp_r: exp_value, .. }) = instr_source.node.node else {
        unreachable!();
    };
    let pl::InstrKind::If(pl::IfInstr { mut block, .. }) = instr_if.node.node else {
        unreachable!();
    };
    let instr_target = block.remove(0);
    let pl::InstrKind::Let(pl::LetInstr { exp_l, .. }) = instr_target.node.node else {
        unreachable!();
    };
    let pl::ExpKind::Opt(Some(exp_target)) = exp_l.node.node else {
        unreachable!();
    };
    Some(crate::annotated! {
        node: crate::note_phrase! {
            node: pl::InstrKind::OptionGet(pl::OptionGetInstr {
                exp_l: *exp_target,
                exp_r: exp_value,
                block,
            }),
            note: instr_source.node.note,
            span: instr_source.node.span,
        },
        hints: instr_source.hints,
    })
}

fn shorten_block_shorthands<Tier>(block: pl::Block<Tier>) -> pl::Block<Tier> {
    let mut instrs_pending = VecDeque::from(block);
    let mut block_output = Vec::new();
    while !instrs_pending.is_empty() {
        if let Some(instr) = shorten_option_get(&mut instrs_pending) {
            block_output.push(instr);
        } else {
            block_output.push(
                instrs_pending
                    .pop_front()
                    .expect("pending instruction was checked as non-empty"),
            );
        }
    }
    block_output
        .into_iter()
        .map(shorten_instr_shorthands)
        .collect()
}

// == Dispatch tier

// - Holding condition

fn shorten_hold_case_dispatch(
    hold_case: pl::HoldCase<pl::InstrDispatch>,
) -> pl::HoldCase<pl::InstrDispatch> {
    match hold_case {
        pl::HoldCase::Both(block_hold, block_not_hold) => {
            let block_hold = shorten_block_dispatch(block_hold);
            let block_not_hold = shorten_block_dispatch(block_not_hold);
            pl::HoldCase::Both(block_hold, block_not_hold)
        }
        pl::HoldCase::Hold(block, dangle) => {
            let block = shorten_block_dispatch(block);
            pl::HoldCase::Hold(block, dangle)
        }
        pl::HoldCase::NotHold(block, dangle) => {
            let block = shorten_block_dispatch(block);
            pl::HoldCase::NotHold(block, dangle)
        }
    }
}

// - Tier instruction

fn shorten_tier_instr_dispatch(instr_dispatch: pl::InstrDispatch) -> pl::InstrDispatch {
    match instr_dispatch {
        pl::InstrDispatch::Group(mut instr_group) => {
            instr_group.block = shorten_block_group(instr_group.block);
            pl::InstrDispatch::Group(instr_group)
        }
        pl::InstrDispatch::Route(mut instr_route) => {
            let mut blocks = Vec::with_capacity(instr_route.blocks.len());
            for block in instr_route.blocks {
                blocks.push(shorten_block_dispatch(block));
            }
            instr_route.blocks = blocks;
            pl::InstrDispatch::Route(instr_route)
        }
    }
}

// - Instruction

fn shorten_instr_dispatch(mut instr: pl::Instr<pl::InstrDispatch>) -> pl::Instr<pl::InstrDispatch> {
    instr.node.node = shorten_instr_kind_dispatch(instr.node.node);
    instr
}

fn shorten_instr_kind_dispatch(
    instr_kind: pl::InstrKind<pl::InstrDispatch>,
) -> pl::InstrKind<pl::InstrDispatch> {
    match instr_kind {
        pl::InstrKind::If(mut instr_if) => {
            instr_if.block = shorten_block_dispatch(instr_if.block);
            pl::InstrKind::If(instr_if)
        }
        pl::InstrKind::Hold(mut instr_hold) => {
            instr_hold.hold_case = shorten_hold_case_dispatch(instr_hold.hold_case);
            pl::InstrKind::Hold(instr_hold)
        }
        pl::InstrKind::Case(mut instr_case) => {
            for case in &mut instr_case.cases {
                case.block = shorten_block_dispatch(std::mem::take(&mut case.block));
            }
            pl::InstrKind::Case(instr_case)
        }
        pl::InstrKind::CheckLetSub(mut instr_check) => {
            instr_check.block = shorten_block_dispatch(instr_check.block);
            pl::InstrKind::CheckLetSub(instr_check)
        }
        pl::InstrKind::CheckLetMatch(mut instr_check) => {
            instr_check.block = shorten_block_dispatch(instr_check.block);
            pl::InstrKind::CheckLetMatch(instr_check)
        }
        pl::InstrKind::OptionGet(mut instr_get) => {
            instr_get.block = shorten_block_dispatch(instr_get.block);
            pl::InstrKind::OptionGet(instr_get)
        }
        pl::InstrKind::Tier(instr_tier) => {
            let tier = shorten_tier_instr_dispatch(instr_tier.tier);
            pl::InstrKind::Tier(pl::TierInstr { tier })
        }
        kind @ (pl::InstrKind::Let(_) | pl::InstrKind::Debug(_) | pl::InstrKind::Destruct(_)) => {
            kind
        }
    }
}

// - Block

fn shorten_block_dispatch(block: pl::BlockDispatch) -> pl::BlockDispatch {
    shorten_block_shorthands(block)
        .into_iter()
        .map(shorten_instr_dispatch)
        .collect()
}

// == Group tier

// - Holding condition

fn shorten_hold_case_group(
    hold_case: pl::HoldCase<pl::InstrGroup>,
) -> pl::HoldCase<pl::InstrGroup> {
    match hold_case {
        pl::HoldCase::Both(block_hold, block_not_hold) => {
            let block_hold = shorten_block_group(block_hold);
            let block_not_hold = shorten_block_group(block_not_hold);
            pl::HoldCase::Both(block_hold, block_not_hold)
        }
        pl::HoldCase::Hold(block, dangle) => {
            let block = shorten_block_group(block);
            pl::HoldCase::Hold(block, dangle)
        }
        pl::HoldCase::NotHold(block, dangle) => {
            let block = shorten_block_group(block);
            pl::HoldCase::NotHold(block, dangle)
        }
    }
}

// - Tier instruction

fn shorten_tier_instr_group(instr_group: pl::InstrGroup) -> pl::InstrGroup {
    match instr_group {
        pl::InstrGroup::Backtrack(mut instr_backtrack) => {
            let mut blocks = Vec::with_capacity(instr_backtrack.blocks.len());
            for block in instr_backtrack.blocks {
                blocks.push(shorten_block_group(block));
            }
            instr_backtrack.blocks = blocks;
            pl::InstrGroup::Backtrack(instr_backtrack)
        }
        instr_group @ (pl::InstrGroup::Result(_)
        | pl::InstrGroup::Return(_)
        | pl::InstrGroup::Rule(_)) => instr_group,
    }
}

// - Instruction

fn shorten_instr_group(mut instr: pl::Instr<pl::InstrGroup>) -> pl::Instr<pl::InstrGroup> {
    instr.node.node = shorten_instr_kind_group(instr.node.node);
    instr
}

fn shorten_instr_kind_group(
    instr_kind: pl::InstrKind<pl::InstrGroup>,
) -> pl::InstrKind<pl::InstrGroup> {
    match instr_kind {
        pl::InstrKind::If(mut instr_if) => {
            instr_if.block = shorten_block_group(instr_if.block);
            pl::InstrKind::If(instr_if)
        }
        pl::InstrKind::Hold(mut instr_hold) => {
            instr_hold.hold_case = shorten_hold_case_group(instr_hold.hold_case);
            pl::InstrKind::Hold(instr_hold)
        }
        pl::InstrKind::Case(mut instr_case) => {
            for case in &mut instr_case.cases {
                case.block = shorten_block_group(std::mem::take(&mut case.block));
            }
            pl::InstrKind::Case(instr_case)
        }
        pl::InstrKind::CheckLetSub(mut instr_check) => {
            instr_check.block = shorten_block_group(instr_check.block);
            pl::InstrKind::CheckLetSub(instr_check)
        }
        pl::InstrKind::CheckLetMatch(mut instr_check) => {
            instr_check.block = shorten_block_group(instr_check.block);
            pl::InstrKind::CheckLetMatch(instr_check)
        }
        pl::InstrKind::OptionGet(mut instr_get) => {
            instr_get.block = shorten_block_group(instr_get.block);
            pl::InstrKind::OptionGet(instr_get)
        }
        pl::InstrKind::Tier(instr_tier) => {
            let tier = shorten_tier_instr_group(instr_tier.tier);
            pl::InstrKind::Tier(pl::TierInstr { tier })
        }
        kind @ (pl::InstrKind::Let(_) | pl::InstrKind::Debug(_) | pl::InstrKind::Destruct(_)) => {
            kind
        }
    }
}

// - Block

fn shorten_block_group(block: pl::BlockGroup) -> pl::BlockGroup {
    shorten_block_shorthands(block)
        .into_iter()
        .map(shorten_instr_group)
        .collect()
}

// == Definitions

fn shorten_def(mut def: pl::Def) -> pl::Def {
    def.node.node = shorten_def_kind(def.node.node);
    def
}

fn shorten_def_kind(def_kind: pl::DefKind) -> pl::DefKind {
    match def_kind {
        pl::DefKind::Rel(def_rel) => {
            let def_rel = shorten_rel_def(def_rel);
            pl::DefKind::Rel(def_rel)
        }
        pl::DefKind::MetaFunc(def_func) => {
            let def_func = shorten_func_def(def_func);
            pl::DefKind::MetaFunc(def_func)
        }
        kind @ (pl::DefKind::Typ(_) | pl::DefKind::Var(_)) => kind,
    }
}

fn shorten_rel_def(def_rel: pl::RelDef) -> pl::RelDef {
    match def_rel {
        pl::RelDef::Defined(mut def_rel) => {
            def_rel.block = shorten_block_dispatch(def_rel.block);
            def_rel.block_else_opt = def_rel.block_else_opt.map(shorten_block_dispatch);
            pl::RelDef::Defined(def_rel)
        }
        pl::RelDef::Extern(def_rel) => pl::RelDef::Extern(def_rel),
    }
}

fn shorten_func_def(def_func: pl::MetaFuncDef) -> pl::MetaFuncDef {
    match def_func {
        pl::MetaFuncDef::Table(mut def_func) => {
            for row in &mut def_func.rows {
                row.block = shorten_block_group(std::mem::take(&mut row.block));
            }
            pl::MetaFuncDef::Table(def_func)
        }
        pl::MetaFuncDef::Defined(mut def_func) => {
            def_func.block = shorten_block_group(def_func.block);
            def_func.block_else_opt = def_func.block_else_opt.map(shorten_block_group);
            pl::MetaFuncDef::Defined(def_func)
        }
        def_func @ (pl::MetaFuncDef::Extern(_) | pl::MetaFuncDef::Builtin(_)) => def_func,
    }
}

// == Entry point

pub(super) fn spec(spec: pl::Spec) -> pl::Spec {
    spec.into_iter().map(shorten_def).collect()
}
