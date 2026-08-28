use crate::lang::{common::ds::map::IdMap, hints::input::InputHint, il::ast};

pub use super::super::types::TypeEnvironment;
use super::{Function, Relation, TypeDimension};

pub type VariableEnvironment = IdMap<TypeDimension>;
pub type MetavariableEnvironment = IdMap<ast::Typ>;
pub type RelationEnvironment = IdMap<Relation>;
pub type InputHintEnvironment = IdMap<InputHint>;
pub type FunctionEnvironment = IdMap<Function>;
