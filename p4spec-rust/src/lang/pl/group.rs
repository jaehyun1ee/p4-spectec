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

/// Collects nested rule groups in depth-first source order
pub fn collect_groups(block: &BlockDispatch) -> Vec<RuleGroup> {
    let mut rule_groups = Vec::new();
    collect_groups_from_block(block, &mut rule_groups);
    rule_groups
}

fn collect_groups_from_block(block: &BlockDispatch, rule_groups: &mut Vec<RuleGroup>) {
    for instr in block {
        collect_groups_from_instr(instr, rule_groups);
    }
}

fn collect_groups_from_instr(instr: &Instr<InstrDispatch>, rule_groups: &mut Vec<RuleGroup>) {
    match &instr.node.node {
        InstrKind::If(IfInstr { block, .. }) => collect_groups_from_block(block, rule_groups),
        InstrKind::Hold(HoldInstr { hold_case, .. }) => match hold_case {
            HoldCase::Both(block_hold, block_not_hold) => {
                collect_groups_from_block(block_hold, rule_groups);
                collect_groups_from_block(block_not_hold, rule_groups);
            }
            HoldCase::Hold(block, _) | HoldCase::NotHold(block, _) => {
                collect_groups_from_block(block, rule_groups);
            }
        },
        InstrKind::Case(CaseInstr { cases, .. }) => {
            for case in cases {
                collect_groups_from_block(&case.block, rule_groups);
            }
        }
        InstrKind::Let(..) | InstrKind::Debug(_) | InstrKind::Destruct(..) => {}
        InstrKind::CheckLetSub(CheckLetSubInstr { block, .. })
        | InstrKind::CheckLetMatch(CheckLetMatchInstr { block, .. })
        | InstrKind::OptionGet(OptionGetInstr { block, .. }) => {
            collect_groups_from_block(block, rule_groups);
        }
        InstrKind::Tier(TierInstr {
            tier: InstrDispatch::Route(RouteDispatchInstr { blocks }),
        }) => {
            for block in blocks {
                collect_groups_from_block(block, rule_groups);
            }
        }
        InstrKind::Tier(TierInstr {
            tier:
                InstrDispatch::Group(GroupDispatchInstr {
                    id_rel,
                    id_group,
                    rel_signature,
                    exps_input,
                    block,
                }),
        }) => rule_groups.push(RuleGroup {
            hints: instr.hints.clone(),
            id_rulegroup: id_group.clone(),
            id_rel: id_rel.clone(),
            rel_signature: rel_signature.clone(),
            exps: exps_input.clone(),
            body: block.clone(),
        }),
    }
}
