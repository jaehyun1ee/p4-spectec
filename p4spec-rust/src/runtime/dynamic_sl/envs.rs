use std::collections::HashMap;

pub use crate::runtime::dynamic::envs::ValueEnv;
use crate::runtime::r#type::envs::TypeDefMap;

use super::{func::Function, rel::Relation};

// Environments

pub type TypeDefEnv = TypeDefMap;
pub type RelationEnv = HashMap<String, Relation>;
pub type FunctionEnv = HashMap<String, Function>;
