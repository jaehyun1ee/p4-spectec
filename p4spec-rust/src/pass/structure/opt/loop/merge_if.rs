//! Merge equal conditions within runs of OL If instructions
//!
//! `merge_identical_if` finds a later equal condition, even across other Ifs;
//! the two bodies are merged at the earlier instruction:
//!
//! ```text
//! if p { return a }; if q { return b }; if p { return c }
//!
//! becomes
//!
//! if p { return a; return c }; if q { return b }
//! ```
//!
//! The search stops at a non-If instruction and requires matching iterators
//! Conditions with unknown overlap remain separate

use std::collections::VecDeque;

use super::super::overlap::{Overlap, overlap_exp};
use crate::{
    lang::{common::source::Phrase, traits::eq::SyntaxEq},
    pass::structure::{StructureError, ol::ast::*, opt::merge::merge_block},
    runtime::envs::algo::TDEnv,
};

fn merge_identical_if(
    tdenv: &TDEnv,
    instr_target: &IfInstr,
    instrs: &VecDeque<Instr>,
) -> Result<Option<usize>, StructureError> {
    let IfInstr {
        exp: exp_target,
        iter_exps: iter_exps_target,
        ..
    } = instr_target;
    for (idx, instr) in instrs.iter().enumerate() {
        let instr_kind = &instr.node;
        let InstrKind::If(instr_if) = instr_kind else {
            break;
        };
        let IfInstr { exp, iter_exps, .. } = instr_if;
        let eq_iter_exps = iter_exps.syntax_eq(iter_exps_target);
        let overlap = overlap_exp(tdenv, exp_target, exp)?;
        if eq_iter_exps && matches!(overlap, Overlap::Identical) {
            return Ok(Some(idx));
        }
    }
    Ok(None)
}

fn merge_if_instr(
    tdenv: &TDEnv,
    mut instr_if: IfInstr,
    instrs: &mut VecDeque<Instr>,
) -> Result<InstrKind, StructureError> {
    while let Some(idx) = merge_identical_if(tdenv, &instr_if, instrs)? {
        let instr_match = instrs.remove(idx).expect("matching instruction exists");
        let instr_kind_match = instr_match.node;
        let InstrKind::If(instr_match) = instr_kind_match else {
            unreachable!()
        };
        let IfInstr {
            block: block_match, ..
        } = instr_match;
        instr_if.block = merge_block(instr_if.block, block_match);
    }
    let IfInstr {
        exp,
        iter_exps,
        block,
    } = instr_if;
    let block = merge_if(tdenv, block)?;
    Ok(InstrKind::If(IfInstr {
        exp,
        iter_exps,
        block,
    }))
}

fn merge_if_hold_instr(tdenv: &TDEnv, instr: HoldInstr) -> Result<InstrKind, StructureError> {
    let HoldInstr {
        id,
        not_exp,
        iter_exps,
        block_hold,
        block_not_hold,
    } = instr;
    let block_hold = merge_if(tdenv, block_hold)?;
    let block_not_hold = merge_if(tdenv, block_not_hold)?;
    Ok(InstrKind::Hold(HoldInstr {
        id,
        not_exp,
        iter_exps,
        block_hold,
        block_not_hold,
    }))
}

fn merge_if_case_instr(tdenv: &TDEnv, instr: CaseInstr) -> Result<InstrKind, StructureError> {
    let CaseInstr { exp, cases, total } = instr;
    let cases = cases
        .into_iter()
        .map(|case| {
            let Case { guard, block } = case;
            Ok(Case {
                guard,
                block: merge_if(tdenv, block)?,
            })
        })
        .collect::<Result<_, StructureError>>()?;
    Ok(InstrKind::Case(CaseInstr { exp, cases, total }))
}

fn merge_if_group_instr(tdenv: &TDEnv, instr: GroupInstr) -> Result<InstrKind, StructureError> {
    let GroupInstr {
        id,
        rel_signature,
        exps,
        block,
    } = instr;
    let block = merge_if(tdenv, block)?;
    Ok(InstrKind::Group(GroupInstr {
        id,
        rel_signature,
        exps,
        block,
    }))
}

fn merge_if_let_instr(tdenv: &TDEnv, instr: LetInstr) -> Result<InstrKind, StructureError> {
    let LetInstr {
        exp_l,
        exp_r,
        iter_instrs,
        block,
    } = instr;
    let block = merge_if(tdenv, block)?;
    Ok(InstrKind::Let(LetInstr {
        exp_l,
        exp_r,
        iter_instrs,
        block,
    }))
}

fn merge_if_rule_instr(tdenv: &TDEnv, instr: RuleInstr) -> Result<InstrKind, StructureError> {
    let RuleInstr {
        id,
        not_exp,
        input_hint,
        iter_instrs,
        block,
    } = instr;
    let block = merge_if(tdenv, block)?;
    Ok(InstrKind::Rule(RuleInstr {
        id,
        not_exp,
        input_hint,
        iter_instrs,
        block,
    }))
}

fn merge_if(tdenv: &TDEnv, block: Block) -> Result<Block, StructureError> {
    let mut block_output = Vec::with_capacity(block.len());
    let mut instrs = VecDeque::from(block);
    while let Some(instr) = instrs.pop_front() {
        let Phrase {
            node: instr_kind,
            span,
            ..
        } = instr;
        let instr_kind = merge_if_instr_kind(tdenv, instr_kind, &mut instrs)?;
        block_output.push(crate::phrase!(node: instr_kind, span: span));
    }
    Ok(block_output)
}

fn merge_if_instr_kind(
    tdenv: &TDEnv,
    instr_kind: InstrKind,
    instrs: &mut VecDeque<Instr>,
) -> Result<InstrKind, StructureError> {
    match instr_kind {
        InstrKind::If(instr) => merge_if_instr(tdenv, instr, instrs),
        InstrKind::Hold(instr) => merge_if_hold_instr(tdenv, instr),
        InstrKind::Case(instr) => merge_if_case_instr(tdenv, instr),
        InstrKind::Group(instr) => merge_if_group_instr(tdenv, instr),
        InstrKind::Let(instr) => merge_if_let_instr(tdenv, instr),
        InstrKind::Rule(instr) => merge_if_rule_instr(tdenv, instr),
        InstrKind::Return(_) | InstrKind::Result(_) | InstrKind::Debug(_) => Ok(instr_kind),
    }
}

pub(crate) fn apply(tdenv: &TDEnv, block: Block) -> Result<Block, StructureError> {
    merge_if(tdenv, block)
}
