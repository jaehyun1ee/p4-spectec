//! Merge bindings, Ifs, and Holds, then form cases until syntax stops changing
//!
//! `if p { A }; if p { B }` becomes `if p { A; B }`
//! A rewrite can expose another merge, so the sequence repeats

pub(crate) mod casify;
pub(crate) mod merge_binding;
pub(crate) mod merge_hold;
pub(crate) mod merge_if;

use crate::{
    pass::structure::{StructureError, ol::ast::Block},
    runtime::envs::algo::TDEnv,
};

// == Optimization

pub(super) fn optimize(tdenv: &TDEnv, mut block: Block) -> Result<Block, StructureError> {
    loop {
        let mut changed = false;
        block = merge_binding::apply(block, &mut changed)?;
        block = merge_if::apply(tdenv, block, &mut changed)?;
        block = merge_hold::apply(block, &mut changed);
        block = casify::apply(tdenv, block, &mut changed)?;
        // Every successful rewrite consumes a sibling, including in nested blocks
        if !changed {
            return Ok(block);
        }
    }
}
