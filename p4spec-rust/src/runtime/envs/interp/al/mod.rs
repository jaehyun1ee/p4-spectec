//! Definition environments used by AL execution

pub mod ast_prepared;

use super::shared::callable::Callable;

use self::ast_prepared as ast;
use std::rc::Rc;

use crate::lang::common::ds::map::IdMap;

pub use super::shared::TDEnv;
pub type REnv = IdMap<Callable<ast::RelDef>>;
pub type FEnv = IdMap<Rc<Callable<ast::MetaFuncDef>>>;
