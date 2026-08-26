//! Rule-group extraction from prose dispatch blocks

use super::{annot, ast::*};

/// A rule group extracted from a dispatch block
#[derive(Clone, Debug, PartialEq)]
pub struct RuleGroup {
    pub hints: annot::Hints,
    pub id_rulegroup: Id,
    pub id_rel: Id,
    pub rel_signature: RelSignature,
    pub exps: Vec<Exp>,
    pub body: BlockGroup,
}

/// Collects rule groups from a dispatch block
pub fn collect_groups(block: &BlockDispatch) -> Vec<RuleGroup> {
    fn collect_instr(instr: &Instr<InstrDispatch>) -> Vec<RuleGroup> {
        match &instr.node.node.kind {
            InstrKind::If(IfInstr { block, .. }) => collect_block(block),
            InstrKind::Hold(HoldInstr { hold_case, .. }) => match hold_case {
                HoldCase::Both(block_hold, block_not_hold) => collect_block(block_hold)
                    .into_iter()
                    .chain(collect_block(block_not_hold))
                    .collect(),
                HoldCase::Hold(block, _) | HoldCase::NotHold(block, _) => collect_block(block),
            },
            InstrKind::Case(CaseInstr { cases, .. }) => cases
                .iter()
                .flat_map(|case| collect_block(&case.block))
                .collect(),
            InstrKind::Let(..) | InstrKind::Debug(_) | InstrKind::Destruct(..) => Vec::new(),
            InstrKind::CheckLetSub(CheckLetSubInstr { block, .. })
            | InstrKind::CheckLetMatch(CheckLetMatchInstr { block, .. })
            | InstrKind::OptionGet(OptionGetInstr { block, .. }) => collect_block(block),
            InstrKind::Tier(TierInstr {
                tier: InstrDispatch::Route(RouteDispatchInstr { blocks }),
            }) => blocks.iter().flat_map(collect_block).collect(),
            InstrKind::Tier(TierInstr {
                tier:
                    InstrDispatch::Group(GroupDispatchInstr {
                        id_rel,
                        id_group,
                        rel_signature,
                        exps_input,
                        block,
                    }),
            }) => vec![RuleGroup {
                hints: instr.hints.clone(),
                id_rulegroup: id_group.clone(),
                id_rel: id_rel.clone(),
                rel_signature: rel_signature.clone(),
                exps: exps_input.clone(),
                body: block.clone(),
            }],
        }
    }

    fn collect_block(block: &BlockDispatch) -> Vec<RuleGroup> {
        block.iter().flat_map(collect_instr).collect()
    }

    collect_block(block)
}
