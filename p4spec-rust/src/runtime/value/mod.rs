//! Immutable executable values shared by frontends and interpreters

mod fresh;
pub mod r#match;
mod ops;

pub use crate::lang::il::ast::{
    Value as ValueRef, ValueCase, ValueField, ValueKind, ValueNode as Value,
};
pub use fresh::Fresh;
pub use ops::{ValueError, ValueTag, get, make};
