//! Immutable executable values shared by frontends and interpreters

mod fresh;
pub mod r#match;
pub mod model;

pub use fresh::Fresh;
pub use model::{
    Value, ValueCase, ValueError, ValueField, ValueKind, ValueRef, ValueTag, get, make,
};
