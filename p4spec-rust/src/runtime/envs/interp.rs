//! Dynamic value environments

use std::rc::Rc;

use crate::lang::{common::ds::map::VarMap, data::value::Value};

pub type VEnv = VarMap<Rc<Value>>;
