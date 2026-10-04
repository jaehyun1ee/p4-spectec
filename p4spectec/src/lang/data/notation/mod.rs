//! Notation of IL and later stages, as trees and as interned shapes
//!
//! A notation form such as `C |- e : t` is a `Mixfix`:
//! atoms (`|-`, `:`) interleaved with argument holes;
//! `Mixop` is the form without arguments.
//! A `Node` is a form without its arguments, held as a `Tree`
//! or interned node by node in a `ShapeArena` (`ShapeKind`, `Shape`);
//! `walk` holds the traversals both representations share.
//! EL uses only atoms, which stay in `common::notation`.

mod arena;
mod error;
mod flat;
mod mixfix;
pub mod mixop;
mod node;
mod tree;
pub mod walk;

pub use arena::ShapeArena;
pub use error::{ArityMismatch, ShapeError};
pub use flat::{Flat, Shape, ShapeKind};
pub use mixfix::{Mixfix, MixfixRef, View};
pub use mixop::Mixop;
pub use node::{AtomPhrase, Node, Repr};
pub use tree::Tree;
