//! Notation of IL and later stages, as trees and as interned shapes
//!
//! A notation form such as `C |- e : t` is a `Node`:
//! atoms (`|-`, `:`) interleaved with argument holes.
//! As a tree it is a `Mixfix`, and `Mixop` is the form without arguments;
//! interned node by node in a `ShapeArena` it is a `ShapeKind`,
//! and a `Split` keeps a `Shape` handle apart from its arguments.
//! `walk` holds the traversals both representations share.
//! EL uses only atoms, which stay in `common::notation`.

mod arena;
mod error;
mod handle;
pub mod mixop;
mod node;
mod split;
mod tree;
pub mod walk;

pub use arena::ShapeArena;
pub use error::{ArityMismatch, ShapeError};
pub use handle::{Handle, Shape, ShapeKind};
pub use node::{AtomPhrase, Node, Repr};
pub use split::Split;
pub use tree::{Mixfix, Mixop, Tree};
