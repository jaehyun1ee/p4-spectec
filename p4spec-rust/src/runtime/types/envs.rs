use crate::lang::common::ds::map::IdMap;

use super::TypeDef;

/// Type definitions keyed by source-insensitive type identifiers
pub type TDEnv = IdMap<TypeDef>;
