pub(super) mod rename_tick;
pub(super) mod revive_underscore;
use super::ol::ast::Block;
use crate::lang::il::ast::{Arg, Exp};
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct RelBody {
    pub exps_match: Vec<Exp>,
    pub block: Block,
    pub block_else: Option<Block>,
}
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct FuncBody {
    pub args_input: Vec<Arg>,
    pub block: Block,
    pub block_else: Option<Block>,
}
