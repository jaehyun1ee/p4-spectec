//! Dynamic value environments

use crate::lang::{common::ds::map::VarMap, data::value::Value};

pub type VEnv = VarMap<Value>;
