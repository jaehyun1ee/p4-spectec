use crate::lang::{common::ds::map::IdMap, hints::input::InputHint, il::ast};

pub use super::super::types::TDEnv;
use super::{Dim, Func, Rel};

pub type VEnv = IdMap<Dim>;
pub type MEnv = IdMap<ast::Typ>;
pub type REnv = IdMap<Rel>;
pub type IHEnv = IdMap<InputHint>;
pub type FEnv = IdMap<Func>;
