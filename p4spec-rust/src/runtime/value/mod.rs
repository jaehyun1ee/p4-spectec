//! Immutable executable values shared by frontends and interpreters

pub mod r#match;
pub mod model;

pub use model::{
    Value, ValueCase, ValueError, ValueField, ValueKind, ValueRef, ValueTag, get, make,
};
