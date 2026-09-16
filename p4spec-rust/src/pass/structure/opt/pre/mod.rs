//! Remove groups when requested, substitute aliases, then turn tests into matches
//!
//! `let y = x { return y }` becomes `return x` before loop rewrites

pub(crate) mod matchify_if_eq_terminal;
pub(crate) mod remove_group;
pub(crate) mod remove_let_alias;

use crate::pass::structure::{StructureError, ol::ast::Block};

// == Optimization

pub(super) fn optimize(block: Block, without_rule_groups: bool) -> Result<Block, StructureError> {
    let block = if without_rule_groups {
        remove_group::apply(block)
    } else {
        block
    };
    let block = remove_let_alias::apply(block)?;
    let block = matchify_if_eq_terminal::apply(block);
    Ok(block)
}
