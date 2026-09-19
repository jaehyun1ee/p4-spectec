//! Shared execution environments and callable frames

pub mod caches;
pub mod callable;
pub mod frame;

use crate::{lang::common::ds::map::IdMap, runtime::typdef::TypeDef};

pub type TDEnv = IdMap<TypeDef>;
