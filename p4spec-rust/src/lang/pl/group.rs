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
    pub body: &'a GroupBlock,
}

// == Collection

/// Collects nested rule groups in depth-first source order
pub fn collect_rulegroups(block: &DispatchBlock) -> Vec<RuleGroup<'_>> {
    let mut rulegroups = Vec::new();
    collect_rulegroups_from_block(block, &mut rulegroups);
    rulegroups
}

fn collect_rulegroups_from_block<'a>(
    block: &'a DispatchBlock,
    rulegroups: &mut Vec<RuleGroup<'a>>,
) {
    for instr in block {
        collect_rulegroups_from_instr(instr, rulegroups);
    }
}

fn collect_rulegroups_from_instr<'a>(
    instr: &'a Instr<DispatchInstr>,
    rulegroups: &mut Vec<RuleGroup<'a>>,
) {
    match &instr.node.node {
        InstrKind::If(IfInstr { block, .. }) => collect_rulegroups_from_block(block, rulegroups),
        InstrKind::Hold(HoldInstr { hold_case, .. }) => match hold_case {
            HoldCase::Both(block_hold, block_not_hold) => {
                collect_rulegroups_from_block(block_hold, rulegroups);
                collect_rulegroups_from_block(block_not_hold, rulegroups);
            }
            HoldCase::Hold(block, _) | HoldCase::NotHold(block, _) => {
                collect_rulegroups_from_block(block, rulegroups);
            }
        },
        InstrKind::Case(CaseInstr { cases, .. }) => {
            for case in cases {
                collect_rulegroups_from_block(&case.block, rulegroups);
            }
        }
        InstrKind::Let(..) | InstrKind::Debug(_) | InstrKind::Destruct(..) => {}
        InstrKind::CheckLetSub(CheckLetSubInstr { block, .. })
        | InstrKind::CheckLetMatch(CheckLetMatchInstr { block, .. })
        | InstrKind::OptionGet(OptionGetInstr { block, .. }) => {
            collect_rulegroups_from_block(block, rulegroups);
        }
        InstrKind::Tier(TierInstr { tier: DispatchInstr::Route(RouteInstr { blocks }) }) => {
            for block in blocks {
                collect_rulegroups_from_block(block, rulegroups);
            }
        }
        InstrKind::Tier(TierInstr {
            tier:
                DispatchInstr::Group(RuleGroupInstr {
                    id_rel,
                    id_group,
                    rel_signature,
                    exps_input,
                    block,
                }),
        }) => rulegroups.push(RuleGroup {
            hints: &instr.hints,
            id_rulegroup: id_group,
            id_rel,
            rel_signature,
            exps: exps_input,
            body: block,
        }),
    }
}
