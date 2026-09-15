//! Definition environments used by SL execution

use std::rc::Rc;

use crate::{
    lang::{common::ds::map::IdMap, sl::ast},
    runtime::typdef::TypeDef,
};

pub type TDEnv = IdMap<TypeDef>;
pub type REnv = IdMap<ast::RelDef>;
pub type FEnv = IdMap<Rc<ast::MetaFuncDef>>;
