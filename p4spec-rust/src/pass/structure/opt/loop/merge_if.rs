//! Merge equal conditions within runs of OL If instructions
//!
//! `find_identical_if` finds a later equal condition, even across other Ifs;
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
//! `find_identical_if` inspects later siblings; it does not rewrite their bodies
//! `merge_block` merges into the current If and retries before entering its body
//! Conditions with unknown overlap remain separate

use std::collections::VecDeque;

use crate::{
    lang::traits::eq::SyntaxEq,
    pass::structure::{
        StructureError,
        ol::ast::*,
        opt::{
            merge,
            overlap::{Overlap, overlap_exp},
        },
    },
    runtime::envs::algo::TDEnv,
};

// == Instructions

fn merge_instr_kind(
    tdenv: &TDEnv,
    instr_kind: InstrKind,
    instrs: &mut VecDeque<Instr>,
    changed: &mut bool,
) -> Result<InstrKind, StructureError> {
    match instr_kind {
        InstrKind::If(instr) => merge_if_instr(tdenv, instr, instrs, changed),
        InstrKind::Hold(instr) => merge_hold_instr(tdenv, instr, changed),
        InstrKind::Case(instr) => merge_case_instr(tdenv, instr, changed),
        InstrKind::Group(instr) => merge_group_instr(tdenv, instr, changed),
        InstrKind::Let(instr) => merge_let_instr(tdenv, instr, changed),
        InstrKind::Rule(instr) => merge_rule_instr(tdenv, instr, changed),
        InstrKind::Return(_) | InstrKind::Result(_) | InstrKind::Debug(_) => Ok(instr_kind),
    }
}

fn merge_block(tdenv: &TDEnv, block: Block, changed: &mut bool) -> Result<Block, StructureError> {
    let mut block_output = Vec::with_capacity(block.len());
    let mut instrs = VecDeque::from(block);
    while let Some(instr) = instrs.pop_front() {
        let instr_kind = merge_instr_kind(tdenv, instr.node, &mut instrs, changed)?;
        let instr = crate::phrase!(node: instr_kind, span: instr.span);
        block_output.push(instr);
    }
    Ok(block_output)
}

// - If instruction

fn find_identical_if(
    tdenv: &TDEnv,
    instr_target: &IfInstr,
    instrs: &VecDeque<Instr>,
) -> Result<Option<usize>, StructureError> {
    let IfInstr { exp: exp_target, iter_exps: iter_exps_target, .. } = instr_target;
    for (idx, instr) in instrs.iter().enumerate() {
        let InstrKind::If(instr_if) = &instr.node else {
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
    changed: &mut bool,
) -> Result<InstrKind, StructureError> {
    while let Some(idx) = find_identical_if(tdenv, &instr_if, instrs)? {
        *changed = true;
        let instr_match = instrs.remove(idx).expect("matching instruction exists");
        let InstrKind::If(instr_match) = instr_match.node else { unreachable!() };
        let IfInstr { block: block_match, .. } = instr_match;
        instr_if.block = merge::merge_block(instr_if.block, block_match);
    }
    let IfInstr { exp, iter_exps, block } = instr_if;
    let block = merge_block(tdenv, block, changed)?;
    let instr = IfInstr { exp, iter_exps, block };
    Ok(InstrKind::If(instr))
}

// - Hold instruction

fn merge_hold_instr(
    tdenv: &TDEnv,
    instr: HoldInstr,
    changed: &mut bool,
) -> Result<InstrKind, StructureError> {
    let HoldInstr { id, not_exp, iter_exps, block_hold, block_not_hold } = instr;
    let block_hold = merge_block(tdenv, block_hold, changed)?;
    let block_not_hold = merge_block(tdenv, block_not_hold, changed)?;
    let instr = HoldInstr { id, not_exp, iter_exps, block_hold, block_not_hold };
    Ok(InstrKind::Hold(instr))
}

// - Case instruction

fn merge_case_instr(
    tdenv: &TDEnv,
    instr: CaseInstr,
    changed: &mut bool,
) -> Result<InstrKind, StructureError> {
    let CaseInstr { exp, cases, total } = instr;
    let cases = cases
        .into_iter()
        .map(|case| {
            let Case { guard, block } = case;
            let block = merge_block(tdenv, block, changed)?;
            let case = Case { guard, block };
            Ok(case)
        })
        .collect::<Result<_, StructureError>>()?;
    let instr = CaseInstr { exp, cases, total };
    Ok(InstrKind::Case(instr))
}

// - Group instruction

fn merge_group_instr(
    tdenv: &TDEnv,
    instr: GroupInstr,
    changed: &mut bool,
) -> Result<InstrKind, StructureError> {
    let GroupInstr { id, rel_signature, exps, block } = instr;
    let block = merge_block(tdenv, block, changed)?;
    let instr = GroupInstr { id, rel_signature, exps, block };
    Ok(InstrKind::Group(instr))
}

// - Let instruction

fn merge_let_instr(
    tdenv: &TDEnv,
    instr: LetInstr,
    changed: &mut bool,
) -> Result<InstrKind, StructureError> {
    let LetInstr { exp_l, exp_r, iter_instrs, block } = instr;
    let block = merge_block(tdenv, block, changed)?;
    let instr = LetInstr { exp_l, exp_r, iter_instrs, block };
    Ok(InstrKind::Let(instr))
}

// - Rule instruction

fn merge_rule_instr(
    tdenv: &TDEnv,
    instr: RuleInstr,
    changed: &mut bool,
) -> Result<InstrKind, StructureError> {
    let RuleInstr { id, not_exp, input_hint, iter_instrs, block } = instr;
    let block = merge_block(tdenv, block, changed)?;
    let instr = RuleInstr { id, not_exp, input_hint, iter_instrs, block };
    Ok(InstrKind::Rule(instr))
}

// == Entry point

pub(crate) fn apply(
    tdenv: &TDEnv,
    block: Block,
    changed: &mut bool,
) -> Result<Block, StructureError> {
    merge_block(tdenv, block, changed)
}
