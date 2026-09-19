//! Stamp partial prose instructions with their failure destination

use crate::lang::pl::{ast as pl, group, partial};

fn stamp_group_hold_case(
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
            instr_hold.hold_case = stamp_group_hold_case(fallthrough, instr_hold.hold_case);
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
            let last = instr_backtrack.blocks.len().saturating_sub(1);
            instr_backtrack.blocks = instr_backtrack
                .blocks
                .into_iter()
                .enumerate()
                .map(|(index, block)| {
                    let destination =
                        if index < last { pl::Fallthrough::FallNext } else { fallthrough.clone() };
                    stamp_block_group(&destination, block)
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

type DispatchFallthroughs = Vec<(String, pl::Fallthrough)>;

fn stamp_dispatch_hold_case(
    destinations: &DispatchFallthroughs,
    hold_case: pl::HoldCase<pl::InstrDispatch>,
) -> pl::HoldCase<pl::InstrDispatch> {
    match hold_case {
        pl::HoldCase::Both(block_hold, block_not_hold) => pl::HoldCase::Both(
            stamp_block_dispatch(destinations, block_hold),
            stamp_block_dispatch(destinations, block_not_hold),
        ),
        pl::HoldCase::Hold(block, dangle) => {
            pl::HoldCase::Hold(stamp_block_dispatch(destinations, block), dangle)
        }
        pl::HoldCase::NotHold(block, dangle) => {
            pl::HoldCase::NotHold(stamp_block_dispatch(destinations, block), dangle)
        }
    }
}

fn stamp_instr_dispatch(
    destinations: &DispatchFallthroughs,
    mut instr: pl::Instr<pl::InstrDispatch>,
) -> pl::Instr<pl::InstrDispatch> {
    instr.node.node = match instr.node.node {
        pl::InstrKind::If(mut instr_if) => {
            instr_if.block = stamp_block_dispatch(destinations, instr_if.block);
            pl::InstrKind::If(instr_if)
        }
        pl::InstrKind::Hold(mut instr_hold) => {
            instr_hold.hold_case = stamp_dispatch_hold_case(destinations, instr_hold.hold_case);
            pl::InstrKind::Hold(instr_hold)
        }
        pl::InstrKind::Case(mut instr_case) => {
            for case in &mut instr_case.cases {
                case.block = stamp_block_dispatch(destinations, std::mem::take(&mut case.block));
            }
            pl::InstrKind::Case(instr_case)
        }
        pl::InstrKind::CheckLetSub(mut instr_check) => {
            instr_check.block = stamp_block_dispatch(destinations, instr_check.block);
            pl::InstrKind::CheckLetSub(instr_check)
        }
        pl::InstrKind::CheckLetMatch(mut instr_check) => {
            instr_check.block = stamp_block_dispatch(destinations, instr_check.block);
            pl::InstrKind::CheckLetMatch(instr_check)
        }
        pl::InstrKind::OptionGet(mut instr_get) => {
            instr_get.block = stamp_block_dispatch(destinations, instr_get.block);
            pl::InstrKind::OptionGet(instr_get)
        }
        pl::InstrKind::Tier(pl::TierInstr { tier: pl::InstrDispatch::Route(mut instr_route) }) => {
            instr_route.blocks = instr_route
                .blocks
                .into_iter()
                .map(|block| stamp_block_dispatch(destinations, block))
                .collect();
            pl::InstrKind::Tier(pl::TierInstr { tier: pl::InstrDispatch::Route(instr_route) })
        }
        pl::InstrKind::Tier(pl::TierInstr { tier: pl::InstrDispatch::Group(mut instr_group) }) => {
            let destination = destinations
                .iter()
                .find(|(id, _)| id == &instr_group.id_group.node)
                .map(|(_, destination)| destination)
                .expect("every dispatched group has a failure destination");
            instr_group.block = stamp_block_group(destination, instr_group.block);
            pl::InstrKind::Tier(pl::TierInstr { tier: pl::InstrDispatch::Group(instr_group) })
        }
        kind @ (pl::InstrKind::Let(_) | pl::InstrKind::Debug(_) | pl::InstrKind::Destruct(_)) => {
            kind
        }
    };
    instr
}

fn stamp_block_dispatch(
    destinations: &DispatchFallthroughs,
    block: pl::BlockDispatch,
) -> pl::BlockDispatch {
    block
        .into_iter()
        .map(|instr| stamp_instr_dispatch(destinations, instr))
        .collect()
}

fn collect_dispatch_fallthroughs(
    final_destination: pl::Fallthrough,
    block: &pl::BlockDispatch,
) -> DispatchFallthroughs {
    let arms = match block.as_slice() {
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
    let groups_by_arm = arms
        .into_iter()
        .map(group::collect_groups)
        .filter(|groups| !groups.is_empty())
        .collect::<Vec<_>>();
    let mut destinations = Vec::new();
    for (index, groups) in groups_by_arm.iter().enumerate() {
        let destination = groups_by_arm
            .get(index + 1)
            .and_then(|groups_next| groups_next.first())
            .map(|group_next| pl::Fallthrough::FallGroup(group_next.id_rulegroup.clone()))
            .unwrap_or_else(|| final_destination.clone());
        destinations.extend(
            groups
                .iter()
                .map(|group| (group.id_rulegroup.node.clone(), destination.clone())),
        );
    }
    destinations
}

fn stamp_def(mut def: pl::Def) -> pl::Def {
    def.node.node = match def.node.node {
        pl::DefKind::Rel(pl::RelDef::Defined(mut def_rel)) => {
            let final_destination = if def_rel
                .block_else_opt
                .as_ref()
                .is_some_and(|block_else| !block_else.is_empty())
            {
                pl::Fallthrough::FallElse
            } else {
                pl::Fallthrough::FallFail
            };
            let destinations = collect_dispatch_fallthroughs(final_destination, &def_rel.block);
            def_rel.block = stamp_block_dispatch(&destinations, def_rel.block);
            pl::DefKind::Rel(pl::RelDef::Defined(def_rel))
        }
        pl::DefKind::MetaFunc(pl::MetaFuncDef::Defined(mut def_func)) => {
            let destination = if def_func
                .block_else_opt
                .as_ref()
                .is_some_and(|block_else| !block_else.is_empty())
            {
                pl::Fallthrough::FallElse
            } else {
                pl::Fallthrough::FallFail
            };
            def_func.block = stamp_block_group(&destination, def_func.block);
            pl::DefKind::MetaFunc(pl::MetaFuncDef::Defined(def_func))
        }
        kind => kind,
    };
    def
}

pub(super) fn spec(spec: pl::Spec) -> pl::Spec {
    spec.into_iter().map(stamp_def).collect()
}
