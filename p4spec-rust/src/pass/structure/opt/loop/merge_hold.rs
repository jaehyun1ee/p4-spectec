//! Merge adjacent OL Hold instructions that test the same relation
//!
//! `merge_hold_instr` merges the holding bodies and the non-holding bodies
//! separately, preserving their order:
//!
//! ```text
//! hold R(x) { return a } else { return b }
//! hold R(x) { return c } else { return d }
//!
//! becomes
//!
//! hold R(x) { return a; return c } else { return b; return d }
//! ```
//!
//! The relation id, arguments, and iterators must match; intervening
//! instructions prevent the merge
//! `take_identical_hold` consumes only the next sibling when it matches
//! After merging H1 with H2, `merge_block` rewrites the remaining siblings
//! before retrying H12: H1; H2; H3; H4 -> H12; H34 -> H1234

use std::collections::VecDeque;

use crate::lang::traits::eq::SyntaxEq;
use crate::pass::structure::{ol::ast::*, opt::merge};

// == Instructions

fn merge_instr_kind(instr_kind: InstrKind, instrs: &mut VecDeque<Instr>) -> (InstrKind, bool) {
    match instr_kind {
        InstrKind::If(instr) => (merge_if_instr(instr), false),
        InstrKind::Hold(instr) => merge_hold_instr(instr, instrs),
        InstrKind::Case(instr) => (merge_case_instr(instr), false),
        InstrKind::Group(instr) => (merge_group_instr(instr), false),
        InstrKind::Let(instr) => (merge_let_instr(instr), false),
        InstrKind::Rule(instr) => (merge_rule_instr(instr), false),
        instr_kind => (instr_kind, false),
    }
}

fn merge_block(block: Block) -> Block {
    let mut instrs: VecDeque<_> = block.into();
    let mut block = Vec::with_capacity(instrs.len());
    let mut blocks_pending: Vec<(Block, Instr)> = Vec::new();
    loop {
        while let Some(instr) = instrs.pop_front() {
            let (instr_kind, merged) = merge_instr_kind(instr.node, &mut instrs);
            let instr = crate::phrase!(node: instr_kind, span: instr.span);
            if merged {
                let block_prefix = std::mem::take(&mut block);
                blocks_pending.push((block_prefix, instr));
            } else {
                block.push(instr);
            }
        }
        let Some((block_prefix, instr)) = blocks_pending.pop() else {
            return block;
        };
        instrs = block.into();
        instrs.push_front(instr);
        block = block_prefix;
    }
}

// - If instruction

fn merge_if_instr(instr: IfInstr) -> InstrKind {
    let IfInstr {
        exp,
        iter_exps,
        block,
    } = instr;
    let block = merge_block(block);
    let instr = IfInstr {
        exp,
        iter_exps,
        block,
    };
    InstrKind::If(instr)
}

// - Hold instruction

fn take_identical_hold(instr_target: &HoldInstr, block: &mut VecDeque<Instr>) -> Option<HoldInstr> {
    let instr_head = block.front()?;
    let InstrKind::Hold(instr_hold) = &instr_head.node else {
        return None;
    };
    if !instr_target.id.syntax_eq(&instr_hold.id)
        || !instr_target.not_exp.syntax_eq(&instr_hold.not_exp)
        || !instr_target.iter_exps.syntax_eq(&instr_hold.iter_exps)
    {
        return None;
    }
    let instr_head = block.pop_front()?;
    let InstrKind::Hold(instr_hold) = instr_head.node else {
        unreachable!()
    };
    Some(instr_hold)
}

fn merge_hold_instr(instr: HoldInstr, instrs: &mut VecDeque<Instr>) -> (InstrKind, bool) {
    let instr_merge = take_identical_hold(&instr, instrs);
    let HoldInstr {
        id,
        not_exp,
        iter_exps,
        block_hold,
        block_not_hold,
    } = instr;
    if let Some(instr_merge) = instr_merge {
        let HoldInstr {
            id,
            not_exp,
            iter_exps,
            block_hold: block_hold_target,
            block_not_hold: block_not_hold_target,
        } = instr_merge;
        let block_hold = merge::merge_block(block_hold, block_hold_target);
        let block_not_hold = merge::merge_block(block_not_hold, block_not_hold_target);
        let instr = HoldInstr {
            id,
            not_exp,
            iter_exps,
            block_hold,
            block_not_hold,
        };
        (InstrKind::Hold(instr), true)
    } else {
        let block_hold = merge_block(block_hold);
        let block_not_hold = merge_block(block_not_hold);
        let instr = HoldInstr {
            id,
            not_exp,
            iter_exps,
            block_hold,
            block_not_hold,
        };
        (InstrKind::Hold(instr), false)
    }
}

// - Case instruction

fn merge_case_instr(instr: CaseInstr) -> InstrKind {
    let CaseInstr { exp, cases, total } = instr;
    let cases = cases
        .into_iter()
        .map(|case| {
            let Case { guard, block } = case;
            let block = merge_block(block);
            Case { guard, block }
        })
        .collect();
    let instr = CaseInstr { exp, cases, total };
    InstrKind::Case(instr)
}

// - Group instruction

fn merge_group_instr(instr: GroupInstr) -> InstrKind {
    let GroupInstr {
        id,
        rel_signature,
        exps,
        block,
    } = instr;
    let block = merge_block(block);
    let instr = GroupInstr {
        id,
        rel_signature,
        exps,
        block,
    };
    InstrKind::Group(instr)
}

// - Let instruction

fn merge_let_instr(instr: LetInstr) -> InstrKind {
    let LetInstr {
        exp_l,
        exp_r,
        iter_instrs,
        block,
    } = instr;
    let block = merge_block(block);
    let instr = LetInstr {
        exp_l,
        exp_r,
        iter_instrs,
        block,
    };
    InstrKind::Let(instr)
}

// - Rule instruction

fn merge_rule_instr(instr: RuleInstr) -> InstrKind {
    let RuleInstr {
        id,
        not_exp,
        input_hint,
        iter_instrs,
        block,
    } = instr;
    let block = merge_block(block);
    let instr = RuleInstr {
        id,
        not_exp,
        input_hint,
        iter_instrs,
        block,
    };
    InstrKind::Rule(instr)
}

// == Entry point

pub(crate) fn apply(block: Block) -> Block {
    merge_block(block)
}
