//! Revive used underscore names and remove ticks until names stop changing
//!
//! An input `_x''` used by `Return(_x'')` becomes `x` and `Return(x)`

pub(super) mod rename_tick;
pub(super) mod revive_underscore;

use super::{StructureError, ol::ast::Block};
use crate::lang::{
    il::ast::{Arg, Exp},
    traits::eq::SyntaxEq,
};

// == Relations

pub(crate) fn pretty_rel(
    mut exps_match: Vec<Exp>,
    mut block: Block,
    mut block_else: Option<Block>,
) -> Result<(Vec<Exp>, Block, Option<Block>), StructureError> {
    loop {
        let body =
            revive_underscore::apply_rel((exps_match.clone(), block.clone(), block_else.clone()))?;
        let (exps_pretty, block_pretty, block_else_pretty) = rename_tick::apply_rel(body)?;
        if exps_match.syntax_eq(&exps_pretty)
            && block.syntax_eq(&block_pretty)
            && block_else.syntax_eq(&block_else_pretty)
        {
            return Ok((exps_match, block, block_else));
        }
        (exps_match, block, block_else) = (exps_pretty, block_pretty, block_else_pretty);
    }
}

// == Functions

pub(crate) fn pretty_func(
    mut args_input: Vec<Arg>,
    mut block: Block,
    mut block_else: Option<Block>,
) -> Result<(Vec<Arg>, Block, Option<Block>), StructureError> {
    loop {
        let body =
            revive_underscore::apply_func((args_input.clone(), block.clone(), block_else.clone()))?;
        let (args_pretty, block_pretty, block_else_pretty) = rename_tick::apply_func(body)?;
        if args_input.syntax_eq(&args_pretty)
            && block.syntax_eq(&block_pretty)
            && block_else.syntax_eq(&block_else_pretty)
        {
            return Ok((args_input, block, block_else));
        }
        (args_input, block, block_else) = (args_pretty, block_pretty, block_else_pretty);
    }
}
