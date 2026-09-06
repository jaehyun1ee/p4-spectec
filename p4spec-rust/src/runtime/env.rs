//! Runtime environments shared by type operations and evaluators

use crate::lang::common::ds::map::IdMap;

use super::typdef::TypeDef;

/// Type definitions keyed by source-insensitive type identifiers
pub type TDEnv = IdMap<TypeDef>;
