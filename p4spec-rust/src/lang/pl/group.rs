//! Rule-group extraction from prose dispatch blocks

use super::{annot, ast::*};

// == Rule groups

/// A borrowed rule group extracted from a dispatch block
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RuleGroup<'a> {
    pub hints: &'a annot::Hints,
    pub id_rulegroup: &'a Id,
    pub id_rel: &'a Id,
    pub rel_signature: &'a RelSignature,
    pub exps: &'a [Exp],
    pub body: &'a BlockGroup,
}

// == Collection

/// Collects nested rule groups in depth-first source order
pub fn collect_groups(block: &BlockDispatch) -> Vec<RuleGroup<'_>> {
    let mut rule_groups = Vec::new();
    collect_groups_from_block(block, &mut rule_groups);
    rule_groups
}

fn collect_groups_from_block<'a>(block: &'a BlockDispatch, rule_groups: &mut Vec<RuleGroup<'a>>) {
    for instr in block {
        collect_groups_from_instr(instr, rule_groups);
    }
}

fn collect_groups_from_instr<'a>(
    instr: &'a Instr<InstrDispatch>,
    rule_groups: &mut Vec<RuleGroup<'a>>,
) {
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
            hints: &instr.hints,
            id_rulegroup: id_group,
            id_rel,
            rel_signature,
            exps: exps_input,
            body: block,
        }),
    }
}
