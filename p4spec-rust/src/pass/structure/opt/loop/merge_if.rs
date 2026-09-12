use super::super::overlap::{Overlap, overlap_exp};
use crate::{
    lang::{
        common::source::{Phrase, Span},
        traits::eq::SyntaxEq,
    },
    pass::structure::{StructureError, merge::merge_block, ol::ast::*},
    runtime::envs::algo::TDEnv,
};

fn merge_identical_if(
    tdenv: &TDEnv,
    instr_target: &IfInstr,
    block: &Block,
) -> Result<Option<usize>, StructureError> {
    let IfInstr {
        exp: exp_target,
        iter_exps: iter_exps_target,
        ..
    } = instr_target;
    for (idx, instr) in block.iter().enumerate() {
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
    instr_if: IfInstr,
    span: Span,
    mut block_tail: Block,
) -> Result<Block, StructureError> {
    if let Some(idx) = merge_identical_if(tdenv, &instr_if, &block_tail)? {
        let instr_match = block_tail.remove(idx);
        let instr_kind_match = instr_match.node;
        let InstrKind::If(instr_match) = instr_kind_match else {
            unreachable!()
        };
        let IfInstr {
            exp,
            iter_exps,
            block,
        } = instr_if;
        let IfInstr {
            block: block_match, ..
        } = instr_match;
        let block = merge_block(block, block_match);
        block_tail.insert(
            0,
            crate::phrase!(node: InstrKind::If(IfInstr { exp, iter_exps, block }), span: span),
        );
        return merge_if(tdenv, block_tail);
    }
    let IfInstr {
        exp,
        iter_exps,
        block,
    } = instr_if;
    let block = merge_if(tdenv, block)?;
    finish_instr(
        tdenv,
        InstrKind::If(IfInstr {
            exp,
            iter_exps,
            block,
        }),
        span,
        block_tail,
    )
}

fn merge_if_hold_instr(
    tdenv: &TDEnv,
    instr: HoldInstr,
    span: Span,
    block_tail: Block,
) -> Result<Block, StructureError> {
    let HoldInstr {
        id,
        not_exp,
        iter_exps,
        block_hold,
        block_not_hold,
    } = instr;
    let block_hold = merge_if(tdenv, block_hold)?;
    let block_not_hold = merge_if(tdenv, block_not_hold)?;
    finish_instr(
        tdenv,
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

fn merge_if_case_instr(
    tdenv: &TDEnv,
    instr: CaseInstr,
    span: Span,
    block_tail: Block,
) -> Result<Block, StructureError> {
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
    finish_instr(
        tdenv,
        InstrKind::Case(CaseInstr { exp, cases, total }),
        span,
        block_tail,
    )
}

fn merge_if_group_instr(
    tdenv: &TDEnv,
    instr: GroupInstr,
    span: Span,
    block_tail: Block,
) -> Result<Block, StructureError> {
    let GroupInstr {
        id,
        rel_signature,
        exps,
        block,
    } = instr;
    let block = merge_if(tdenv, block)?;
    finish_instr(
        tdenv,
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

fn merge_if_let_instr(
    tdenv: &TDEnv,
    instr: LetInstr,
    span: Span,
    block_tail: Block,
) -> Result<Block, StructureError> {
    let LetInstr {
        exp_l,
        exp_r,
        iter_instrs,
        block,
    } = instr;
    let block = merge_if(tdenv, block)?;
    finish_instr(
        tdenv,
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

fn merge_if_rule_instr(
    tdenv: &TDEnv,
    instr: RuleInstr,
    span: Span,
    block_tail: Block,
) -> Result<Block, StructureError> {
    let RuleInstr {
        id,
        not_exp,
        input_hint,
        iter_instrs,
        block,
    } = instr;
    let block = merge_if(tdenv, block)?;
    finish_instr(
        tdenv,
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

fn finish_instr(
    tdenv: &TDEnv,
    instr_kind: InstrKind,
    span: Span,
    block_tail: Block,
) -> Result<Block, StructureError> {
    let instr = crate::phrase!(node: instr_kind, span: span);
    let mut block = vec![instr];
    block.extend(merge_if(tdenv, block_tail)?);
    Ok(block)
}
fn merge_if(tdenv: &TDEnv, mut block: Block) -> Result<Block, StructureError> {
    if block.is_empty() {
        return Ok(block);
    }
    let instr = block.remove(0);
    let Phrase {
        node: instr_kind,
        note: (),
        span,
    } = instr;
    merge_if_instr_kind(tdenv, instr_kind, span, block)
}
fn merge_if_instr_kind(
    tdenv: &TDEnv,
    instr_kind: InstrKind,
    span: Span,
    block_tail: Block,
) -> Result<Block, StructureError> {
    match instr_kind {
        InstrKind::If(instr) => merge_if_instr(tdenv, instr, span, block_tail),
        InstrKind::Hold(instr) => merge_if_hold_instr(tdenv, instr, span, block_tail),
        InstrKind::Case(instr) => merge_if_case_instr(tdenv, instr, span, block_tail),
        InstrKind::Group(instr) => merge_if_group_instr(tdenv, instr, span, block_tail),
        InstrKind::Let(instr) => merge_if_let_instr(tdenv, instr, span, block_tail),
        InstrKind::Rule(instr) => merge_if_rule_instr(tdenv, instr, span, block_tail),
        InstrKind::Return(_) | InstrKind::Result(_) | InstrKind::Debug(_) => {
            finish_instr(tdenv, instr_kind, span, block_tail)
        }
    }
}
pub(crate) fn apply(tdenv: &TDEnv, block: Block) -> Result<Block, StructureError> {
    merge_if(tdenv, block)
}
