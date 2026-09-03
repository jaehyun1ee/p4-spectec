//! Dynamic state shared by interpreters

mod caches;
mod envs;
mod var;

pub use caches::{CallCache, CallKey, ValueCache};
pub use envs::{TDEnv, VEnv};
pub use var::Variable;
