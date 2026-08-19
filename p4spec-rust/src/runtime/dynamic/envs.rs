use hashbrown::HashMap;

use crate::runtime::{r#type::envs::TypeDefMap, value::ValueRef};

use super::var::Variable;

// Value environment

pub type ValueEnv = HashMap<Variable, ValueRef>;

// Type definition environment

pub type TypeDefEnv = TypeDefMap;
