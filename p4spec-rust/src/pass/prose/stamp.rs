//! Stamp partial prose instructions with their failure destination

use crate::lang::pl::{ast as pl, group, partial};

type FallthroughsByRuleGroup = Vec<(String, pl::Fallthrough)>;

// == Group tier

// - Holding condition

fn stamp_group_hold_case(
    fallthrough: &pl::Fallthrough,
    hold_case: pl::HoldCase<pl::GroupInstr>,
) -> pl::HoldCase<pl::GroupInstr> {
    match hold_case {
        pl::HoldCase::Both(block_hold, block_not_hold) => {
            let block_hold = stamp_group_block(fallthrough, block_hold);
            let block_not_hold = stamp_group_block(fallthrough, block_not_hold);
            pl::HoldCase::Both(block_hold, block_not_hold)
        }
        pl::HoldCase::Hold(block, dangle) => {
            let block = stamp_group_block(fallthrough, block);
            pl::HoldCase::Hold(block, dangle)
        }
        pl::HoldCase::NotHold(block, dangle) => {
            let block = stamp_group_block(fallthrough, block);
            pl::HoldCase::NotHold(block, dangle)
        }
    }
}

// - Tier instruction

fn stamp_backtrack_instr(
    fallthrough: &pl::Fallthrough,
    mut instr_backtrack: pl::BacktrackInstr,
) -> pl::BacktrackInstr {
    let idx_last = instr_backtrack.blocks.len().saturating_sub(1);
    let mut blocks = Vec::with_capacity(instr_backtrack.blocks.len());
    for (idx, block) in instr_backtrack.blocks.into_iter().enumerate() {
        let fallthrough_block =
            if idx < idx_last { pl::Fallthrough::FallNext } else { fallthrough.clone() };
        blocks.push(stamp_group_block(&fallthrough_block, block));
    }
    instr_backtrack.blocks = blocks;
    instr_backtrack
}

fn stamp_group_tier(fallthrough: &pl::Fallthrough, instr_group: pl::GroupInstr) -> pl::GroupInstr {
    match instr_group {
        pl::GroupInstr::Backtrack(instr_backtrack) => {
            let instr_backtrack = stamp_backtrack_instr(fallthrough, instr_backtrack);
            pl::GroupInstr::Backtrack(instr_backtrack)
        }
        instr_group @ (pl::GroupInstr::Result(_)
        | pl::GroupInstr::Return(_)
        | pl::GroupInstr::Rule(_)) => instr_group,
    }
}

// - Instruction

fn stamp_group_instr(
    fallthrough: &pl::Fallthrough,
    mut instr: pl::Instr<pl::GroupInstr>,
) -> pl::Instr<pl::GroupInstr> {
    if partial::is_partial_instr(partial::is_partial_group_instr, &instr) {
        instr.node.note = Some(fallthrough.clone());
    }
    instr.node.node = stamp_group_instr_kind(fallthrough, instr.node.node);
    instr
}

fn stamp_group_instr_kind(
    fallthrough: &pl::Fallthrough,
    instr_kind: pl::InstrKind<pl::GroupInstr>,
) -> pl::InstrKind<pl::GroupInstr> {
    match instr_kind {
        pl::InstrKind::If(mut instr_if) => {
            instr_if.block = stamp_group_block(fallthrough, instr_if.block);
            pl::InstrKind::If(instr_if)
        }
        pl::InstrKind::Hold(mut instr_hold) => {
            instr_hold.hold_case = stamp_group_hold_case(fallthrough, instr_hold.hold_case);
            pl::InstrKind::Hold(instr_hold)
        }
        pl::InstrKind::Case(mut instr_case) => {
            for case in &mut instr_case.cases {
                case.block = stamp_group_block(fallthrough, std::mem::take(&mut case.block));
            }
            pl::InstrKind::Case(instr_case)
        }
        pl::InstrKind::CheckLetSub(mut instr_check) => {
            instr_check.block = stamp_group_block(fallthrough, instr_check.block);
            pl::InstrKind::CheckLetSub(instr_check)
        }
        pl::InstrKind::CheckLetMatch(mut instr_check) => {
            instr_check.block = stamp_group_block(fallthrough, instr_check.block);
            pl::InstrKind::CheckLetMatch(instr_check)
        }
        pl::InstrKind::OptionGet(mut instr_get) => {
            instr_get.block = stamp_group_block(fallthrough, instr_get.block);
            pl::InstrKind::OptionGet(instr_get)
        }
        pl::InstrKind::Tier(instr_tier) => {
            let tier = stamp_group_tier(fallthrough, instr_tier.tier);
            pl::InstrKind::Tier(pl::TierInstr { tier })
        }
        kind @ (pl::InstrKind::Let(_) | pl::InstrKind::Debug(_) | pl::InstrKind::Destruct(_)) => {
            kind
        }
    }
}

// - Block

fn stamp_group_block(fallthrough: &pl::Fallthrough, block: pl::GroupBlock) -> pl::GroupBlock {
    block
        .into_iter()
        .map(|instr| stamp_group_instr(fallthrough, instr))
        .collect()
}

// == Dispatch tier

// - Holding condition

fn stamp_dispatch_hold_case(
    fallthroughs_by_rulegroup: &FallthroughsByRuleGroup,
    hold_case: pl::HoldCase<pl::DispatchInstr>,
) -> pl::HoldCase<pl::DispatchInstr> {
    match hold_case {
        pl::HoldCase::Both(block_hold, block_not_hold) => {
            let block_hold = stamp_dispatch_block(fallthroughs_by_rulegroup, block_hold);
            let block_not_hold = stamp_dispatch_block(fallthroughs_by_rulegroup, block_not_hold);
            pl::HoldCase::Both(block_hold, block_not_hold)
        }
        pl::HoldCase::Hold(block, dangle) => {
            let block = stamp_dispatch_block(fallthroughs_by_rulegroup, block);
            pl::HoldCase::Hold(block, dangle)
        }
        pl::HoldCase::NotHold(block, dangle) => {
            let block = stamp_dispatch_block(fallthroughs_by_rulegroup, block);
            pl::HoldCase::NotHold(block, dangle)
        }
    }
}

// - Tier instruction

fn stamp_route_instr(
    fallthroughs_by_rulegroup: &FallthroughsByRuleGroup,
    mut instr_route: pl::RouteInstr,
) -> pl::RouteInstr {
    let mut blocks = Vec::with_capacity(instr_route.blocks.len());
    for block in instr_route.blocks {
        blocks.push(stamp_dispatch_block(fallthroughs_by_rulegroup, block));
    }
    instr_route.blocks = blocks;
    instr_route
}

fn stamp_rulegroup_instr(
    fallthroughs_by_rulegroup: &FallthroughsByRuleGroup,
    mut instr_group: pl::RuleGroupInstr,
) -> pl::RuleGroupInstr {
    let fallthrough = fallthroughs_by_rulegroup
        .iter()
        .find(|(id_group, _)| id_group == &instr_group.id_group.node)
        .map(|(_, fallthrough)| fallthrough)
        .expect("every dispatched group has a failure destination");
    instr_group.block = stamp_group_block(fallthrough, instr_group.block);
    instr_group
}

fn stamp_dispatch_tier(
    fallthroughs_by_rulegroup: &FallthroughsByRuleGroup,
    instr_dispatch: pl::DispatchInstr,
) -> pl::DispatchInstr {
    match instr_dispatch {
        pl::DispatchInstr::Route(instr_route) => {
            let instr_route = stamp_route_instr(fallthroughs_by_rulegroup, instr_route);
            pl::DispatchInstr::Route(instr_route)
        }
        pl::DispatchInstr::Group(instr_group) => {
            let instr_group = stamp_rulegroup_instr(fallthroughs_by_rulegroup, instr_group);
            pl::DispatchInstr::Group(instr_group)
        }
    }
}

// - Instruction

fn stamp_dispatch_instr(
    fallthroughs_by_rulegroup: &FallthroughsByRuleGroup,
    mut instr: pl::Instr<pl::DispatchInstr>,
) -> pl::Instr<pl::DispatchInstr> {
    instr.node.node = stamp_dispatch_instr_kind(fallthroughs_by_rulegroup, instr.node.node);
    instr
}

fn stamp_dispatch_instr_kind(
    fallthroughs_by_rulegroup: &FallthroughsByRuleGroup,
    instr_kind: pl::InstrKind<pl::DispatchInstr>,
) -> pl::InstrKind<pl::DispatchInstr> {
    match instr_kind {
        pl::InstrKind::If(mut instr_if) => {
            instr_if.block = stamp_dispatch_block(fallthroughs_by_rulegroup, instr_if.block);
            pl::InstrKind::If(instr_if)
        }
        pl::InstrKind::Hold(mut instr_hold) => {
            instr_hold.hold_case =
                stamp_dispatch_hold_case(fallthroughs_by_rulegroup, instr_hold.hold_case);
            pl::InstrKind::Hold(instr_hold)
        }
        pl::InstrKind::Case(mut instr_case) => {
            for case in &mut instr_case.cases {
                case.block = stamp_dispatch_block(
                    fallthroughs_by_rulegroup,
                    std::mem::take(&mut case.block),
                );
            }
            pl::InstrKind::Case(instr_case)
        }
        pl::InstrKind::CheckLetSub(mut instr_check) => {
            instr_check.block = stamp_dispatch_block(fallthroughs_by_rulegroup, instr_check.block);
            pl::InstrKind::CheckLetSub(instr_check)
        }
        pl::InstrKind::CheckLetMatch(mut instr_check) => {
            instr_check.block = stamp_dispatch_block(fallthroughs_by_rulegroup, instr_check.block);
            pl::InstrKind::CheckLetMatch(instr_check)
        }
        pl::InstrKind::OptionGet(mut instr_get) => {
            instr_get.block = stamp_dispatch_block(fallthroughs_by_rulegroup, instr_get.block);
            pl::InstrKind::OptionGet(instr_get)
        }
        pl::InstrKind::Tier(instr_tier) => {
            let tier = stamp_dispatch_tier(fallthroughs_by_rulegroup, instr_tier.tier);
            pl::InstrKind::Tier(pl::TierInstr { tier })
        }
        kind @ (pl::InstrKind::Let(_) | pl::InstrKind::Debug(_) | pl::InstrKind::Destruct(_)) => {
            kind
        }
    }
}

// - Block

fn stamp_dispatch_block(
    fallthroughs_by_rulegroup: &FallthroughsByRuleGroup,
    block: pl::DispatchBlock,
) -> pl::DispatchBlock {
    block
        .into_iter()
        .map(|instr| stamp_dispatch_instr(fallthroughs_by_rulegroup, instr))
        .collect()
}

// == Dispatch fallthroughs

fn collect_fallthroughs_by_rulegroup(
    fallthrough_final: pl::Fallthrough,
    block: &pl::DispatchBlock,
) -> FallthroughsByRuleGroup {
    let blocks_dispatch = match block.as_slice() {
        [instr]
            if matches!(
                instr.node.node,
                pl::InstrKind::Tier(pl::TierInstr { tier: pl::DispatchInstr::Route(_) })
            ) =>
        {
            let pl::InstrKind::Tier(pl::TierInstr { tier: pl::DispatchInstr::Route(instr_route) }) =
                &instr.node.node
            else {
                unreachable!();
            };
            instr_route.blocks.iter().collect::<Vec<_>>()
        }
        _ => vec![block],
    };
    let rulegroups_by_dispatch_arm = blocks_dispatch
        .into_iter()
        .map(group::collect_rulegroups)
        .filter(|rulegroups| !rulegroups.is_empty())
        .collect::<Vec<_>>();
    let mut fallthroughs_by_rulegroup = Vec::new();
    for (idx, rulegroups) in rulegroups_by_dispatch_arm.iter().enumerate() {
        let fallthrough = rulegroups_by_dispatch_arm
            .get(idx + 1)
            .and_then(|rulegroups_next| rulegroups_next.first())
            .map(|rulegroup_next| pl::Fallthrough::FallGroup(rulegroup_next.id_rulegroup.clone()))
            .unwrap_or_else(|| fallthrough_final.clone());
        fallthroughs_by_rulegroup.extend(
            rulegroups
                .iter()
                .map(|rulegroup| (rulegroup.id_rulegroup.node.clone(), fallthrough.clone())),
        );
    }
    fallthroughs_by_rulegroup
}

// == Definitions

fn stamp_def(mut def: pl::Def) -> pl::Def {
    def.node.node = stamp_def_kind(def.node.node);
    def
}

fn stamp_def_kind(def_kind: pl::DefKind) -> pl::DefKind {
    match def_kind {
        pl::DefKind::Rel(def_rel) => {
            let def_rel = stamp_rel_def(def_rel);
            pl::DefKind::Rel(def_rel)
        }
        pl::DefKind::MetaFunc(def_func) => {
            let def_func = stamp_func_def(def_func);
            pl::DefKind::MetaFunc(def_func)
        }
        kind @ (pl::DefKind::Typ(_) | pl::DefKind::Var(_)) => kind,
    }
}

fn stamp_rel_def(def_rel: pl::RelDef) -> pl::RelDef {
    match def_rel {
        pl::RelDef::Defined(def_rel) => {
            let def_rel = stamp_defined_rel_def(def_rel);
            pl::RelDef::Defined(def_rel)
        }
        pl::RelDef::Extern(def_rel) => pl::RelDef::Extern(def_rel),
    }
}

fn stamp_defined_rel_def(mut def_rel: pl::DefinedRel) -> pl::DefinedRel {
    let fallthrough_final = if def_rel
        .block_else_opt
        .as_ref()
        .is_some_and(|block_else| !block_else.is_empty())
    {
        pl::Fallthrough::FallElse
    } else {
        pl::Fallthrough::FallFail
    };
    let fallthroughs_by_rulegroup =
        collect_fallthroughs_by_rulegroup(fallthrough_final, &def_rel.block);
    def_rel.block = stamp_dispatch_block(&fallthroughs_by_rulegroup, def_rel.block);
    def_rel
}

fn stamp_func_def(def_func: pl::MetaFuncDef) -> pl::MetaFuncDef {
    match def_func {
        pl::MetaFuncDef::Defined(def_func) => {
            let def_func = stamp_defined_func_def(def_func);
            pl::MetaFuncDef::Defined(def_func)
        }
        def_func @ (pl::MetaFuncDef::Extern(_)
        | pl::MetaFuncDef::Builtin(_)
        | pl::MetaFuncDef::Table(_)) => def_func,
    }
}

fn stamp_defined_func_def(mut def_func: pl::DefinedFunc) -> pl::DefinedFunc {
    let fallthrough = if def_func
        .block_else_opt
        .as_ref()
        .is_some_and(|block_else| !block_else.is_empty())
    {
        pl::Fallthrough::FallElse
    } else {
        pl::Fallthrough::FallFail
    };
    def_func.block = stamp_group_block(&fallthrough, def_func.block);
    def_func
}

// == Entry point

pub(super) fn spec(spec: pl::Spec) -> pl::Spec {
    spec.into_iter().map(stamp_def).collect()
}
