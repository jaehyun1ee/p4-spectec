//! Run pre-rewrites, ordered loop rewrites to syntax equality, then post-rewrites
use super::{
    StructureError,
    ol::ast::Block,
    opt::{r#loop, post, pre},
};
use crate::{lang::traits::eq::SyntaxEq, runtime::envs::algo::TDEnv};

fn optimize_pre(block: Block, without_rule_groups: bool) -> Result<Block, StructureError> {
    let block = if without_rule_groups {
        pre::remove_group::apply(block)
    } else {
        block
    };
    let block = pre::remove_let_alias::apply(block)?;
    Ok(pre::matchify_if_eq_terminal::apply(block))
}
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
fn optimize_post(tdenv: &TDEnv, block: Block) -> Result<Block, StructureError> {
    let block = post::remove_let_dead::apply(block)?;
    post::remove_match_singleton::apply(tdenv, block)
}
pub(super) fn optimize(
    tdenv: &TDEnv,
    block: Block,
    without_rule_groups: bool,
) -> Result<Block, StructureError> {
    let block = optimize_pre(block, without_rule_groups)?;
    let block = optimize_loop(tdenv, block)?;
    optimize_post(tdenv, block)
}
pub(super) struct Blocks {
    pub block: Block,
    pub block_else: Option<Block>,
}
pub(super) fn optimize_with_else(
    tdenv: &TDEnv,
    block: Block,
    block_else: Option<Block>,
    without_rule_groups: bool,
) -> Result<Blocks, StructureError> {
    let block = optimize(tdenv, block, without_rule_groups)?;
    let block_else = block_else
        .map(|block_else| optimize(tdenv, block_else, without_rule_groups))
        .transpose()?;
    Ok(Blocks { block, block_else })
}
pub(super) fn optimize_without_else(
    tdenv: &TDEnv,
    block: Block,
    without_rule_groups: bool,
) -> Result<Block, StructureError> {
    optimize(tdenv, block, without_rule_groups)
}
