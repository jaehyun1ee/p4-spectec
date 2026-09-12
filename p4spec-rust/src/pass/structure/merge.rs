use super::ol::ast::{Block, IfInstr, Instr, InstrKind};
use crate::lang::{common::source::Phrase, traits::eq::SyntaxEq};

pub(crate) fn merge_block(mut block_a: Block, mut block_b: Block) -> Block {
    if block_a.is_empty() {
        return block_b;
    }
    if block_b.is_empty() {
        return block_a;
    }

    let instr_a = block_a.remove(0);
    let instr_b = block_b.remove(0);
    let Phrase {
        node: instr_kind_a,
        note: (),
        span: span_a,
    } = instr_a;
    let Phrase {
        node: instr_kind_b,
        note: (),
        span: span_b,
    } = instr_b;
    merge_instr_heads(instr_kind_a, span_a, block_a, instr_kind_b, span_b, block_b)
}

fn merge_instr_heads(
    instr_kind_a: InstrKind,
    span_a: crate::lang::common::source::Span,
    block_tail_a: Block,
    instr_kind_b: InstrKind,
    span_b: crate::lang::common::source::Span,
    block_tail_b: Block,
) -> Block {
    match (instr_kind_a, instr_kind_b) {
        (InstrKind::If(instr_if_a), InstrKind::If(instr_if_b)) => merge_if_instrs(
            instr_if_a,
            span_a,
            block_tail_a,
            instr_if_b,
            span_b,
            block_tail_b,
        ),
        (instr_kind_a, instr_kind_b) => concatenate(
            Phrase {
                node: instr_kind_a,
                note: (),
                span: span_a,
            },
            block_tail_a,
            Phrase {
                node: instr_kind_b,
                note: (),
                span: span_b,
            },
            block_tail_b,
        ),
    }
}

fn merge_if_instrs(
    instr_if_a: IfInstr,
    span_a: crate::lang::common::source::Span,
    block_tail_a: Block,
    instr_if_b: IfInstr,
    span_b: crate::lang::common::source::Span,
    block_tail_b: Block,
) -> Block {
    let IfInstr {
        exp: exp_cond_a,
        iter_exps: iter_exps_a,
        block: block_if_a,
    } = instr_if_a;
    let IfInstr {
        exp: exp_cond_b,
        iter_exps: iter_exps_b,
        block: block_if_b,
    } = instr_if_b;

    if exp_cond_a.syntax_eq(&exp_cond_b) && iter_exps_a.syntax_eq(&iter_exps_b) {
        let block_if = merge_block(block_if_a, block_if_b);
        let instr_if = Phrase {
            node: InstrKind::If(IfInstr {
                exp: exp_cond_a,
                iter_exps: iter_exps_a,
                block: block_if,
            }),
            note: (),
            span: span_a,
        };
        let mut block_a = Vec::with_capacity(1 + block_tail_a.len());
        block_a.push(instr_if);
        block_a.extend(block_tail_a);
        merge_block(block_a, block_tail_b)
    } else {
        let instr_a = Phrase {
            node: InstrKind::If(IfInstr {
                exp: exp_cond_a,
                iter_exps: iter_exps_a,
                block: block_if_a,
            }),
            note: (),
            span: span_a,
        };
        let instr_b = Phrase {
            node: InstrKind::If(IfInstr {
                exp: exp_cond_b,
                iter_exps: iter_exps_b,
                block: block_if_b,
            }),
            note: (),
            span: span_b,
        };
        concatenate(instr_a, block_tail_a, instr_b, block_tail_b)
    }
}

fn concatenate(instr_a: Instr, block_tail_a: Block, instr_b: Instr, block_tail_b: Block) -> Block {
    let mut block = Vec::with_capacity(2 + block_tail_a.len() + block_tail_b.len());
    block.push(instr_a);
    block.extend(block_tail_a);
    block.push(instr_b);
    block.extend(block_tail_b);
    block
}

pub(crate) fn merge_blocks(mut blocks: Vec<Block>) -> Block {
    if blocks.is_empty() {
        return vec![];
    }
    let block = blocks.remove(0);
    blocks.into_iter().fold(block, merge_block)
}
