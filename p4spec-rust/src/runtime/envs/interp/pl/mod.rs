//! Definition environments used by PL execution
//!
//! Callables contain prepared PL control flow and a frame layout.
//! Prepared expressions retain their prose annotations until evaluation.

pub mod ast_prepared;

use ast_prepared as ast;
use std::rc::Rc;

use crate::{lang::common::ds::map::IdMap, runtime::envs::interp::shared::callable::Callable};

pub use super::shared::TDEnv;

/// Relations to their prepared callables.
pub type REnv = IdMap<Callable<ast::RelDef>>;
/// Functions to their prepared callables, shared through `Rc`.
pub type FEnv = IdMap<Rc<Callable<ast::MetaFuncDef>>>;
