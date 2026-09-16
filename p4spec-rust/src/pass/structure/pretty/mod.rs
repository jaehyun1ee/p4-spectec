//! Revive used underscore names and remove ticks until names stop changing
//!
//! An input `_x''` used by `Return(_x'')` becomes `x` and `Return(x)`

pub(super) mod rename_tick;
pub(super) mod revive_underscore;

use std::{cell::Cell, rc::Rc};

use super::{StructureError, ol::ast::Block};
use crate::lang::il::ast::{Arg, Exp};

// Revival removes leading underscores; tick cleanup shortens suffixes
// Actual name changes cannot cancel within a round, including iterator names

// == Relations

pub(crate) fn pretty_rel(
    exps_match: Vec<Exp>,
    block: Block,
    block_else: Option<Block>,
) -> Result<(Vec<Exp>, Block, Option<Block>), StructureError> {
    let changed = Rc::new(Cell::new(false));
    let mut body = (exps_match, block, block_else);
    loop {
        changed.set(false);
        body = revive_underscore::apply_rel(body, &changed)?;
        body = rename_tick::apply_rel(body, &changed)?;
        if !changed.get() {
            return Ok(body);
        }
    }
}

// == Functions

pub(crate) fn pretty_func(
    args_input: Vec<Arg>,
    block: Block,
    block_else: Option<Block>,
) -> Result<(Vec<Arg>, Block, Option<Block>), StructureError> {
    let changed = Rc::new(Cell::new(false));
    let mut body = (args_input, block, block_else);
    loop {
        changed.set(false);
        body = revive_underscore::apply_func(body, &changed)?;
        body = rename_tick::apply_func(body, &changed)?;
        if !changed.get() {
            return Ok(body);
        }
    }
}
