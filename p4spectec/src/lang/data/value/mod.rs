//! Values of IL and later stages, as arena handles and as trees
//!
//! `flat::Value` (`ValueFlat`) holds body, type, and span handles
//! into one `Arena`; `tree::Value` (`ValueTree`) owns those contents.
//! The representation names are also exported directly by this module.
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
pub use flat::{ValueCase, ValueField, ValueFlat, ValueFlatKind, ValueTag};
pub use tree::{ValueTree, ValueTreeKind};
pub use view::ValueRef;
