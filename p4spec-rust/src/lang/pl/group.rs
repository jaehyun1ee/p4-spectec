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

pub fn collect_groups(block: &BlockDispatch) -> Vec<RuleGroup> {
    fn collect_instr(instr: &Instr<InstrDispatch>) -> Vec<RuleGroup> {
        match &instr.node.kind {
            InstrKind::IfI(_, _, block, _) => collect_block(block),
            InstrKind::HoldI(_, _, _, holdcase) => match holdcase {
                HoldCase::BothH(block_hold, block_not_hold) => collect_block(block_hold)
                    .into_iter()
                    .chain(collect_block(block_not_hold))
                    .collect(),
                HoldCase::HoldH(block, _) | HoldCase::NotHoldH(block, _) => collect_block(block),
            },
            InstrKind::CaseI(_, cases, _) => cases
                .iter()
                .flat_map(|case| collect_block(&case.block))
                .collect(),
            InstrKind::LetI(..) | InstrKind::DebugI(_) | InstrKind::DestructI(..) => Vec::new(),
            InstrKind::CheckLetSubI(_, _, _, _, block)
            | InstrKind::CheckLetMatchI(_, _, _, block)
            | InstrKind::OptionGetI(_, _, block) => collect_block(block),
            InstrKind::TierI(InstrDispatch::RouteI(arms)) => {
                arms.iter().flat_map(collect_block).collect()
            }
            InstrKind::TierI(InstrDispatch::GroupI {
                group_id,
                relation_id,
                signature,
                inputs,
                block,
            }) => vec![RuleGroup {
                hints: instr.hints.clone(),
                id_rulegroup: group_id.clone(),
                id_rel: relation_id.clone(),
                rel_signature: signature.clone(),
                exps: inputs.clone(),
                body: block.clone(),
            }],
        }
    }

    fn collect_block(block: &BlockDispatch) -> Vec<RuleGroup> {
        block.iter().flat_map(collect_instr).collect()
    }

    collect_block(block)
}
