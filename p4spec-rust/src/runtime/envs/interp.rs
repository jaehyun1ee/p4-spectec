//! Shared execution type and value environments

use crate::{
    lang::{
        common::ds::map::{IdMap, VarMap},
        data::value::Value,
    },
    runtime::typdef::TypeDef,
};

pub type TDEnv = IdMap<TypeDef>;
pub type VEnv = VarMap<Value>;
