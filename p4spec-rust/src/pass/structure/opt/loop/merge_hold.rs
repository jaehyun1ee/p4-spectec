//! Merge adjacent Hold conditions and recursively combine both outcomes
use crate::lang::{
    common::source::{Phrase, Span},
    traits::eq::SyntaxEq,
};
use crate::pass::structure::{merge::merge_block, ol::ast::*};

fn merge_identical_hold(instr_target: &HoldInstr, block: &mut Block) -> Option<HoldInstr> {
    let instr_head = block.first()?;
    let instr_kind = &instr_head.node;
    let InstrKind::Hold(instr_hold) = instr_kind else {
        return None;
    };
    if !instr_target.id.syntax_eq(&instr_hold.id)
        || !instr_target.not_exp.syntax_eq(&instr_hold.not_exp)
        || !instr_target.iter_exps.syntax_eq(&instr_hold.iter_exps)
    {
        return None;
    }
    let instr_head = block.remove(0);
    let Phrase {
        node: instr_kind, ..
    } = instr_head;
    let InstrKind::Hold(instr_hold) = instr_kind else {
        unreachable!()
    };
    Some(instr_hold)
}

fn merge_hold(block: Block) -> Block {
    let mut instrs = block.into_iter();
    let Some(instr_head) = instrs.next() else {
        return vec![];
    };
    let Phrase {
        node: instr_kind,
        span,
        note: (),
    } = instr_head;
    let block_tail = instrs.collect();
    match instr_kind {
        InstrKind::If(instr_if) => merge_if_instr(instr_if, span, block_tail),
        InstrKind::Hold(instr_hold) => merge_hold_instr(instr_hold, span, block_tail),
        InstrKind::Case(instr_case) => merge_case_instr(instr_case, span, block_tail),
        InstrKind::Group(instr_group) => merge_group_instr(instr_group, span, block_tail),
        InstrKind::Let(instr_let) => merge_let_instr(instr_let, span, block_tail),
        InstrKind::Rule(instr_rule) => merge_rule_instr(instr_rule, span, block_tail),
        instr_kind => finish(instr_kind, span, block_tail),
    }
}
fn finish(instr_kind: InstrKind, span: Span, block_tail: Block) -> Block {
    let mut block = vec![Phrase {
        node: instr_kind,
        span,
        note: (),
    }];
    block.extend(merge_hold(block_tail));
    block
}
fn merge_if_instr(instr_if: IfInstr, span: Span, block_tail: Block) -> Block {
    let IfInstr {
        exp,
        iter_exps,
        block,
    } = instr_if;
    let block = merge_hold(block);
    finish(
        InstrKind::If(IfInstr {
            exp,
            iter_exps,
            block,
        }),
        span,
        block_tail,
    )
}
fn merge_hold_instr(instr_hold: HoldInstr, span: Span, mut block_tail: Block) -> Block {
    let instr_merge = merge_identical_hold(&instr_hold, &mut block_tail);
    let HoldInstr {
        id,
        not_exp,
        iter_exps,
        block_hold,
        block_not_hold,
    } = instr_hold;
    if let Some(instr_merge) = instr_merge {
        let HoldInstr {
            id,
            not_exp,
            iter_exps,
            block_hold: block_hold_target,
            block_not_hold: block_not_hold_target,
        } = instr_merge;
        let block_hold = merge_block(block_hold, block_hold_target);
        let block_not_hold = merge_block(block_not_hold, block_not_hold_target);
        // The source adopts the downstream condition and upstream wrapper span
        let instr_kind = InstrKind::Hold(HoldInstr {
            id,
            not_exp,
            iter_exps,
            block_hold,
            block_not_hold,
        });
        let mut block = vec![Phrase {
            node: instr_kind,
            span,
            note: (),
        }];
        block.extend(merge_hold(block_tail));
        merge_hold(block)
    } else {
        let block_hold = merge_hold(block_hold);
        let block_not_hold = merge_hold(block_not_hold);
        finish(
            InstrKind::Hold(HoldInstr {
                id,
                not_exp,
                iter_exps,
                block_hold,
                block_not_hold,
            }),
            span,
            block_tail,
        )
    }
}
fn merge_case_instr(instr_case: CaseInstr, span: Span, block_tail: Block) -> Block {
    let CaseInstr { exp, cases, total } = instr_case;
    let cases = cases
        .into_iter()
        .map(|case| {
            let Case { guard, block } = case;
            let block = merge_hold(block);
            Case { guard, block }
        })
        .collect();
    finish(
        InstrKind::Case(CaseInstr { exp, cases, total }),
        span,
        block_tail,
    )
}
fn merge_group_instr(instr_group: GroupInstr, span: Span, block_tail: Block) -> Block {
    let GroupInstr {
        id,
        rel_signature,
        exps,
        block,
    } = instr_group;
    let block = merge_hold(block);
    finish(
        InstrKind::Group(GroupInstr {
            id,
            rel_signature,
            exps,
            block,
        }),
        span,
        block_tail,
    )
}
fn merge_let_instr(instr_let: LetInstr, span: Span, block_tail: Block) -> Block {
    let LetInstr {
        exp_l,
        exp_r,
        iter_instrs,
        block,
    } = instr_let;
    let block = merge_hold(block);
    finish(
        InstrKind::Let(LetInstr {
            exp_l,
            exp_r,
            iter_instrs,
            block,
        }),
        span,
        block_tail,
    )
}
fn merge_rule_instr(instr_rule: RuleInstr, span: Span, block_tail: Block) -> Block {
    let RuleInstr {
        id,
        not_exp,
        input_hint,
        iter_instrs,
        block,
    } = instr_rule;
    let block = merge_hold(block);
    finish(
        InstrKind::Rule(RuleInstr {
            id,
            not_exp,
            input_hint,
            iter_instrs,
            block,
        }),
        span,
        block_tail,
    )
}
pub(crate) fn apply(block: Block) -> Block {
    merge_hold(block)
}
