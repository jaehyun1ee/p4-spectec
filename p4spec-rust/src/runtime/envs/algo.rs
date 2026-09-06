//! Environments used by algorithmic conversion

use crate::{
    lang::{common::ds::map::IdMap, il::ast},
    runtime::dim::Dim,
};

pub type VEnv = IdMap<Dim>;
pub type MEnv = IdMap<ast::Typ>;
