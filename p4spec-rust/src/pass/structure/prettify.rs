use super::{
    StructureError,
    ol::ast::Block,
    pretty::{FuncBody, RelBody, rename_tick, revive_underscore},
};
use crate::lang::{
    il::ast::{Arg, Exp},
    traits::eq::SyntaxEq,
};
fn eq_else(block_a: &Option<Block>, block_b: &Option<Block>) -> bool {
    match (block_a, block_b) {
        (Some(block_a), Some(block_b)) => block_a.syntax_eq(block_b),
        (None, None) => true,
        _ => false,
    }
}

pub(crate) fn pretty_rel(
    exps_match: Vec<Exp>,
    block: Block,
    block_else: Option<Block>,
) -> Result<RelBody, StructureError> {
    let mut body = RelBody {
        exps_match,
        block,
        block_else,
    };
    loop {
        let body_pretty = revive_underscore::apply_rel(body.clone())?;
        let body_pretty = rename_tick::apply_rel(body_pretty)?;
        if body.exps_match.syntax_eq(&body_pretty.exps_match)
            && body.block.syntax_eq(&body_pretty.block)
            && eq_else(&body.block_else, &body_pretty.block_else)
        {
            // Stable syntax retains the previous source and proof metadata
            return Ok(body);
        }
        body = body_pretty;
    }
}

pub(crate) fn pretty_func(
    args_input: Vec<Arg>,
    block: Block,
    block_else: Option<Block>,
) -> Result<FuncBody, StructureError> {
    let mut body = FuncBody {
        args_input,
        block,
        block_else,
    };
    loop {
        let body_pretty = revive_underscore::apply_func(body.clone())?;
        let body_pretty = rename_tick::apply_func(body_pretty)?;
        if body.args_input.syntax_eq(&body_pretty.args_input)
            && body.block.syntax_eq(&body_pretty.block)
            && eq_else(&body.block_else, &body_pretty.block_else)
        {
            // Stable syntax retains the previous source and proof metadata
            return Ok(body);
        }
        body = body_pretty;
    }
}
