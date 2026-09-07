//! Environments used by elaboration

use crate::{
    lang::{common::ds::map::IdMap, hints::input::InputHint, il::ast},
    runtime::dim::Dim,
    runtime::typdef::TypeDef,
};

pub type TDEnv = IdMap<TypeDef>;
pub type VEnv = IdMap<Dim>;
pub type MEnv = IdMap<ast::Typ>;
pub type REnv = IdMap<ast::RelDef>;
pub type IHEnv = IdMap<InputHint>;
pub type FEnv = IdMap<ast::MetaFuncDef>;
