use super::ol::ast::{Block, IfInstr, Instr, InstrKind};
use crate::lang::traits::eq::SyntaxEq;

// == Block merging

pub(crate) fn merge_block(mut block_a: Block, mut block_b: Block) -> Block {
    if let (Some(instr_a), Some(instr_b)) = (block_a.first_mut(), block_b.first_mut())
        && merge_instr_heads(instr_a, instr_b)
    {
        block_b.remove(0);
        return merge_block(block_a, block_b);
    }
    block_a.extend(block_b);
    block_a
}

// - Common instruction heads

fn merge_instr_heads(instr_a: &mut Instr, instr_b: &mut Instr) -> bool {
    match (&mut instr_a.node, &mut instr_b.node) {
        (InstrKind::If(instr_if_a), InstrKind::If(instr_if_b)) => {
            merge_if_instrs(instr_if_a, instr_if_b)
        }
        _ => false,
    }
}

fn merge_if_instrs(instr_if_a: &mut IfInstr, instr_if_b: &mut IfInstr) -> bool {
    if !instr_if_a.exp.syntax_eq(&instr_if_b.exp)
        || !instr_if_a.iter_exps.syntax_eq(&instr_if_b.iter_exps)
    {
        return false;
    }
    let block_a = std::mem::take(&mut instr_if_a.block);
    let block_b = std::mem::take(&mut instr_if_b.block);
    instr_if_a.block = merge_block(block_a, block_b);
    true
}

// == Entry point

pub(crate) fn merge_blocks(mut blocks: Vec<Block>) -> Block {
    if blocks.is_empty() {
        return vec![];
    }
    let block = blocks.remove(0);
    blocks.into_iter().fold(block, merge_block)
}
