//! Definition environments used by PL execution
//!
//! The source PL definitions remain annotated; each callable carries only its
//! interpreter-owned frame layout alongside that syntax.

use std::rc::Rc;

use crate::{
    lang::{common::ds::map::IdMap, pl::ast},
    runtime::envs::interp::shared::callable::Callable,
};

pub use super::shared::TDEnv;

pub type REnv = IdMap<Callable<ast::RelDef>>;
pub type FEnv = IdMap<Rc<Callable<ast::MetaFuncDef>>>;
