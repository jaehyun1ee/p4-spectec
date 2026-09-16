//! Definition environments used by SL execution

use std::rc::Rc;

use crate::lang::{common::ds::map::IdMap, sl::ast};

pub use super::interp::TDEnv;
pub type REnv = IdMap<ast::RelDef>;
pub type FEnv = IdMap<Rc<ast::MetaFuncDef>>;
