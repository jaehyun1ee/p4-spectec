//! Values of IL and later stages, as arena handles and as trees
//!
//! `flat::Value` holds body, type, and span handles into one `Arena`.
//! `tree::Value` owns those contents and annotations.
//! Each representation defines its own `Value` and `ValueKind`.
//! A case body is its notation shape with its arguments (`ValueCase`).
//! `make` allocates values of each kind with their type,
//! `get` projects a kind back out or fails with `ValueError`.

mod arena;
mod error;
pub mod external;
pub mod flat;
pub mod get;
pub mod make;
pub mod print;
pub mod tree;
mod view;

pub(super) use arena::ValueArena;
pub use error::ValueError;
pub use flat::{ValueCase, ValueField, ValueTag};
pub use view::ValueRef;
