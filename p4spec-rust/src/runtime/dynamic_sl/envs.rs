use std::{collections::HashMap, rc::Rc};

pub use crate::runtime::dynamic::envs::ValueEnv;
use crate::runtime::r#type::envs::TypeDefMap;

use super::{func::Function, rel::Relation};

// Environments

pub type TypeDefEnv = TypeDefMap;
pub type RelationEnv = HashMap<String, Rc<Relation>>;
pub type FunctionEnv = HashMap<String, Rc<Function>>;
