//! Environments used by elaboration and algorithmic conversion

use crate::{
    lang::{common::ds::map::IdMap, hints::input::InputHint, il::ast},
    runtime::{dim::Dim, func::r#static::Func, rel::r#static::Rel},
};

pub type VEnv = IdMap<Dim>;
pub type MEnv = IdMap<ast::Typ>;
pub type REnv = IdMap<Rel>;
pub type IHEnv = IdMap<InputHint>;
pub type FEnv = IdMap<Func>;
