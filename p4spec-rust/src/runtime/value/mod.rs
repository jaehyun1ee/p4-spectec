//! Immutable executable values shared by frontends and interpreters

pub mod r#match;
mod ops;

pub use crate::lang::il::ast::{Value, ValueCase, ValueField, ValueKind};
pub use ops::{ValueError, ValueTag, get, make};
