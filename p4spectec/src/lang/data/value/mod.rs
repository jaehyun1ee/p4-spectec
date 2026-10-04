//! Values of IL and later stages, as arena handles and as trees
//!
//! A `Value` is a handle into an `Arena`.
//! `ValueNode<R>` is one value body: `Handle` holds arena handles
//! (`ValueKind`), `Tree` holds trees that any arena can intern
//! (`tree::ValueKind`), mirroring the representations of `data::notation`.
//! A case body is its notation shape with its arguments (`ValueCase`).
//! `make` allocates values of each kind with their type,
//! `get` projects a kind back out or fails with `ValueError`.

mod arena;
mod args;
mod error;
pub mod external;
pub mod get;
mod handle;
pub mod make;
mod node;
mod primitive;
mod session;
pub mod tree;
mod view;

pub use arena::Arena;
pub use args::ValueArgs;
pub use error::ValueError;
pub use handle::{Handle, Value, ValueCase, ValueField, ValueKind};
pub use node::{ValueNode, ValueRepr, ValueTag};
pub(crate) use session::CaseSession;
pub use tree::Tree;
pub use view::ValueRef;
