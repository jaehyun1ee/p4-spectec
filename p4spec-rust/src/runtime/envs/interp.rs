//! Dynamic value and type environments

use std::rc::Rc;

use imbl::{GenericOrdMap, shared_ptr::RcK};

use crate::{lang::data::value::Value, runtime::var::Variable};

pub type VEnv = GenericOrdMap<Variable, Rc<Value>, RcK>;
