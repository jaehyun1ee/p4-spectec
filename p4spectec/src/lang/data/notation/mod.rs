//! Notation of IL and later stages
//!
//! A notation form such as `C |- e : t` is a `Node`:
//! atoms (`|-`, `:`) interleaved with argument holes.
//! As a tree it is a `Mixfix`, and `Mixop` is the form without arguments.
//! `walk` holds the traversals every representation shares.
//! EL uses only atoms, which stay in `common::notation`.

mod error;
pub mod mixop;
mod node;
mod tree;
pub mod walk;

pub use error::ArityMismatch;
pub use node::{AtomPhrase, Node, Repr};
pub use tree::{Mixfix, Mixop, Tree};
