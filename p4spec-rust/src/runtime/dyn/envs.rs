//! Dynamic value and type environments.

use std::collections::BTreeMap;

use crate::runtime::value::ValueRef;

use super::Variable;

pub type VEnv = BTreeMap<Variable, ValueRef>;
pub use crate::runtime::types::TDEnv;
