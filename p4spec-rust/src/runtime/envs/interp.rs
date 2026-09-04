//! Dynamic value and type environments

use std::{collections::BTreeMap, rc::Rc};

use crate::{lang::data::value::Value, runtime::var::Variable};

pub type VEnv = BTreeMap<Variable, Rc<Value>>;
