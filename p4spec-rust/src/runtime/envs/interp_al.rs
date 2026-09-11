//! Definition environments used by AL execution

use std::rc::Rc;

use crate::{
    lang::{al::ast, common::ds::map::IdMap},
    runtime::typdef::TypeDef,
};

pub type TDEnv = IdMap<TypeDef>;
pub type REnv = IdMap<ast::RelDef>;
pub type FEnv = IdMap<Rc<ast::MetaFuncDef>>;
