//! Optimize OL blocks with pre-rewrites, repeated loop rewrites, then post-rewrites
//!
//! `let y = x { return y }` becomes `return x` during pre-rewrites
//! Loop rewrites repeat until syntax equality; post-rewrites then run once

pub(super) mod r#loop;
pub(super) mod overlap;
pub(super) mod post;
pub(super) mod pre;

use super::{StructureError, ol::ast::Block};
use crate::{lang::traits::eq::SyntaxEq, runtime::envs::algo::TDEnv};

// == Optimization

pub(super) fn optimize(
    tdenv: &TDEnv,
    block: Block,
    without_rule_groups: bool,
) -> Result<Block, StructureError> {
    let block = optimize_pre(block, without_rule_groups)?;
    let block = optimize_loop(tdenv, block)?;
    optimize_post(tdenv, block)
}

// - Pre-rewrites

fn optimize_pre(block: Block, without_rule_groups: bool) -> Result<Block, StructureError> {
    let block = if without_rule_groups {
        pre::remove_group::apply(block)
    } else {
        block
    };
    let block = pre::remove_let_alias::apply(block)?;
    Ok(pre::matchify_if_eq_terminal::apply(block))
}

// - Loop rewrites

fn optimize_loop(tdenv: &TDEnv, mut block: Block) -> Result<Block, StructureError> {
    loop {
        let block_optimized = r#loop::merge_binding::apply(block.clone())?;
        let block_optimized = r#loop::merge_if::apply(tdenv, block_optimized)?;
        let block_optimized = r#loop::merge_hold::apply(block_optimized);
        let block_optimized = r#loop::casify::apply(tdenv, block_optimized)?;
        if block.syntax_eq(&block_optimized) {
            return Ok(block);
        }
        block = block_optimized;
    }
}

// - Post-rewrites

fn optimize_post(tdenv: &TDEnv, block: Block) -> Result<Block, StructureError> {
    let block = post::remove_let_dead::apply(block)?;
    post::remove_match_singleton::apply(tdenv, block)
}
