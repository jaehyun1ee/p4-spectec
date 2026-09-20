//! Stamp partial prose instructions with their failure destination

use crate::lang::pl::{ast as pl, group, partial};

// == Group tier

fn stamp_hold_case_group(
    fallthrough: &pl::Fallthrough,
    hold_case: pl::HoldCase<pl::InstrGroup>,
) -> pl::HoldCase<pl::InstrGroup> {
    match hold_case {
        pl::HoldCase::Both(block_hold, block_not_hold) => pl::HoldCase::Both(
            stamp_block_group(fallthrough, block_hold),
            stamp_block_group(fallthrough, block_not_hold),
        ),
        pl::HoldCase::Hold(block, dangle) => {
            pl::HoldCase::Hold(stamp_block_group(fallthrough, block), dangle)
        }
        pl::HoldCase::NotHold(block, dangle) => {
            pl::HoldCase::NotHold(stamp_block_group(fallthrough, block), dangle)
        }
    }
}

fn stamp_instr_group(
    fallthrough: &pl::Fallthrough,
    mut instr: pl::Instr<pl::InstrGroup>,
) -> pl::Instr<pl::InstrGroup> {
    if partial::is_partial_instr(partial::is_partial_instr_group, &instr) {
        instr.node.note = Some(fallthrough.clone());
    }
    instr.node.node = match instr.node.node {
        pl::InstrKind::If(mut instr_if) => {
            instr_if.block = stamp_block_group(fallthrough, instr_if.block);
            pl::InstrKind::If(instr_if)
        }
        pl::InstrKind::Hold(mut instr_hold) => {
            instr_hold.hold_case = stamp_hold_case_group(fallthrough, instr_hold.hold_case);
            pl::InstrKind::Hold(instr_hold)
        }
        pl::InstrKind::Case(mut instr_case) => {
            for case in &mut instr_case.cases {
                case.block = stamp_block_group(fallthrough, std::mem::take(&mut case.block));
            }
            pl::InstrKind::Case(instr_case)
        }
        pl::InstrKind::CheckLetSub(mut instr_check) => {
            instr_check.block = stamp_block_group(fallthrough, instr_check.block);
            pl::InstrKind::CheckLetSub(instr_check)
        }
        pl::InstrKind::CheckLetMatch(mut instr_check) => {
            instr_check.block = stamp_block_group(fallthrough, instr_check.block);
            pl::InstrKind::CheckLetMatch(instr_check)
        }
        pl::InstrKind::OptionGet(mut instr_get) => {
            instr_get.block = stamp_block_group(fallthrough, instr_get.block);
            pl::InstrKind::OptionGet(instr_get)
        }
        pl::InstrKind::Tier(pl::TierInstr {
            tier: pl::InstrGroup::Backtrack(mut instr_backtrack),
        }) => {
            let idx_last = instr_backtrack.blocks.len().saturating_sub(1);
            instr_backtrack.blocks = instr_backtrack
                .blocks
                .into_iter()
                .enumerate()
                .map(|(idx, block)| {
                    let fallthrough = if idx < idx_last {
                        pl::Fallthrough::FallNext
                    } else {
                        fallthrough.clone()
                    };
                    stamp_block_group(&fallthrough, block)
                })
                .collect();
            pl::InstrKind::Tier(pl::TierInstr { tier: pl::InstrGroup::Backtrack(instr_backtrack) })
        }
        kind @ (pl::InstrKind::Let(_)
        | pl::InstrKind::Debug(_)
        | pl::InstrKind::Destruct(_)
        | pl::InstrKind::Tier(_)) => kind,
    };
    instr
}

fn stamp_block_group(fallthrough: &pl::Fallthrough, block: pl::BlockGroup) -> pl::BlockGroup {
    block
        .into_iter()
        .map(|instr| stamp_instr_group(fallthrough, instr))
        .collect()
}

type FallthroughsByRuleGroup = Vec<(String, pl::Fallthrough)>;

// == Dispatch tier

fn stamp_hold_case_dispatch(
    fallthroughs_by_rulegroup: &FallthroughsByRuleGroup,
    hold_case: pl::HoldCase<pl::InstrDispatch>,
) -> pl::HoldCase<pl::InstrDispatch> {
    match hold_case {
        pl::HoldCase::Both(block_hold, block_not_hold) => pl::HoldCase::Both(
            stamp_block_dispatch(fallthroughs_by_rulegroup, block_hold),
            stamp_block_dispatch(fallthroughs_by_rulegroup, block_not_hold),
        ),
        pl::HoldCase::Hold(block, dangle) => {
            pl::HoldCase::Hold(stamp_block_dispatch(fallthroughs_by_rulegroup, block), dangle)
        }
        pl::HoldCase::NotHold(block, dangle) => {
            pl::HoldCase::NotHold(stamp_block_dispatch(fallthroughs_by_rulegroup, block), dangle)
        }
    }
}

fn stamp_instr_dispatch(
    fallthroughs_by_rulegroup: &FallthroughsByRuleGroup,
    mut instr: pl::Instr<pl::InstrDispatch>,
) -> pl::Instr<pl::InstrDispatch> {
    instr.node.node = match instr.node.node {
        pl::InstrKind::If(mut instr_if) => {
            instr_if.block = stamp_block_dispatch(fallthroughs_by_rulegroup, instr_if.block);
            pl::InstrKind::If(instr_if)
        }
        pl::InstrKind::Hold(mut instr_hold) => {
            instr_hold.hold_case =
                stamp_hold_case_dispatch(fallthroughs_by_rulegroup, instr_hold.hold_case);
            pl::InstrKind::Hold(instr_hold)
        }
        pl::InstrKind::Case(mut instr_case) => {
            for case in &mut instr_case.cases {
                case.block = stamp_block_dispatch(
                    fallthroughs_by_rulegroup,
                    std::mem::take(&mut case.block),
                );
            }
            pl::InstrKind::Case(instr_case)
        }
        pl::InstrKind::CheckLetSub(mut instr_check) => {
            instr_check.block = stamp_block_dispatch(fallthroughs_by_rulegroup, instr_check.block);
            pl::InstrKind::CheckLetSub(instr_check)
        }
        pl::InstrKind::CheckLetMatch(mut instr_check) => {
            instr_check.block = stamp_block_dispatch(fallthroughs_by_rulegroup, instr_check.block);
            pl::InstrKind::CheckLetMatch(instr_check)
        }
        pl::InstrKind::OptionGet(mut instr_get) => {
            instr_get.block = stamp_block_dispatch(fallthroughs_by_rulegroup, instr_get.block);
            pl::InstrKind::OptionGet(instr_get)
        }
        pl::InstrKind::Tier(pl::TierInstr { tier: pl::InstrDispatch::Route(mut instr_route) }) => {
            instr_route.blocks = instr_route
                .blocks
                .into_iter()
                .map(|block| stamp_block_dispatch(fallthroughs_by_rulegroup, block))
                .collect();
            pl::InstrKind::Tier(pl::TierInstr { tier: pl::InstrDispatch::Route(instr_route) })
        }
        pl::InstrKind::Tier(pl::TierInstr { tier: pl::InstrDispatch::Group(mut instr_group) }) => {
            let fallthrough = fallthroughs_by_rulegroup
                .iter()
                .find(|(id_group, _)| id_group == &instr_group.id_group.node)
                .map(|(_, fallthrough)| fallthrough)
                .expect("every dispatched group has a failure destination");
            instr_group.block = stamp_block_group(fallthrough, instr_group.block);
            pl::InstrKind::Tier(pl::TierInstr { tier: pl::InstrDispatch::Group(instr_group) })
        }
        kind @ (pl::InstrKind::Let(_) | pl::InstrKind::Debug(_) | pl::InstrKind::Destruct(_)) => {
            kind
        }
    };
    instr
}

fn stamp_block_dispatch(
    fallthroughs_by_rulegroup: &FallthroughsByRuleGroup,
    block: pl::BlockDispatch,
) -> pl::BlockDispatch {
    block
        .into_iter()
        .map(|instr| stamp_instr_dispatch(fallthroughs_by_rulegroup, instr))
        .collect()
}

// == Dispatch fallthroughs

fn collect_fallthroughs_by_rulegroup(
    fallthrough_final: pl::Fallthrough,
    block: &pl::BlockDispatch,
) -> FallthroughsByRuleGroup {
    let blocks_dispatch = match block.as_slice() {
        [instr]
            if matches!(
                instr.node.node,
                pl::InstrKind::Tier(pl::TierInstr { tier: pl::InstrDispatch::Route(_) })
            ) =>
        {
            let pl::InstrKind::Tier(pl::TierInstr { tier: pl::InstrDispatch::Route(instr_route) }) =
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
    def.node.node = match def.node.node {
        pl::DefKind::Rel(pl::RelDef::Defined(mut def_rel)) => {
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
            def_rel.block = stamp_block_dispatch(&fallthroughs_by_rulegroup, def_rel.block);
            pl::DefKind::Rel(pl::RelDef::Defined(def_rel))
        }
        pl::DefKind::MetaFunc(pl::MetaFuncDef::Defined(mut def_func)) => {
            let fallthrough = if def_func
                .block_else_opt
                .as_ref()
                .is_some_and(|block_else| !block_else.is_empty())
            {
                pl::Fallthrough::FallElse
            } else {
                pl::Fallthrough::FallFail
            };
            def_func.block = stamp_block_group(&fallthrough, def_func.block);
            pl::DefKind::MetaFunc(pl::MetaFuncDef::Defined(def_func))
        }
        kind => kind,
    };
    def
}

// == Entry point

pub(super) fn spec(spec: pl::Spec) -> pl::Spec {
    spec.into_iter().map(stamp_def).collect()
}
