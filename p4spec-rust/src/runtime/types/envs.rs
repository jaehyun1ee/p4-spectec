use crate::lang::common::ds::map::IdMap;

use super::TypeDefinition;

/// Type definitions keyed by source-insensitive type identifiers
pub type TypeEnvironment = IdMap<TypeDefinition>;
