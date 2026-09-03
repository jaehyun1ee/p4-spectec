//! Dynamic value and type environments.

use std::{collections::BTreeMap, rc::Rc};

use crate::runtime::value::Value;

use super::Variable;

pub type VEnv = BTreeMap<Variable, Rc<Value>>;
pub use crate::runtime::types::TDEnv;
