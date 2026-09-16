//! Merge bindings, Ifs, and Holds, then form cases until syntax stops changing
//!
//! `if p { A }; if p { B }` becomes `if p { A; B }`
//! A rewrite can expose another merge, so the sequence repeats

pub(crate) mod casify;
pub(crate) mod merge_binding;
pub(crate) mod merge_hold;
pub(crate) mod merge_if;

use crate::{
    lang::traits::eq::SyntaxEq,
    pass::structure::{StructureError, ol::ast::Block},
    runtime::envs::algo::TDEnv,
};

// == Optimization

pub(super) fn optimize(tdenv: &TDEnv, mut block: Block) -> Result<Block, StructureError> {
    loop {
        let block_optimized = merge_binding::apply(block.clone())?;
        let block_optimized = merge_if::apply(tdenv, block_optimized)?;
        let block_optimized = merge_hold::apply(block_optimized);
        let block_optimized = casify::apply(tdenv, block_optimized)?;
        if block.syntax_eq(&block_optimized) {
            return Ok(block);
        }
        block = block_optimized;
    }
}
