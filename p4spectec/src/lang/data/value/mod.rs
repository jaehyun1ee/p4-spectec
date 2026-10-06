//! Values of IL and later stages, as arena handles and as trees
//!
//! `Value` is the runtime name for `ValueFlat`,
//! holding body, type, and span handles into one `Arena`.
//! `ValueTree` owns the corresponding contents and annotations.
//! A case body is its notation shape with its arguments (`ValueCase`).
//! `make` allocates values of each kind with their type,
//! `get` projects a kind back out or fails with `ValueError`.

mod arena;
mod error;
pub mod external;
mod flat;
pub mod get;
pub mod make;
pub mod print;
pub mod tree;
mod view;

pub(super) use arena::ValueArena;
pub use error::ValueError;
/// The runtime value whose body, type, and span belong to one arena.
pub use flat::ValueFlat as Value;
pub use flat::{ValueCase, ValueField, ValueFlat, ValueFlatKind, ValueTag};
pub use tree::{ValueTree, ValueTreeKind};
pub use view::ValueRef;
