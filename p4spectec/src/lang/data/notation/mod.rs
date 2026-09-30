//! Notation of IL and later stages, as trees and as interned shapes
//!
//! A notation form such as `C |- e : t` is a `Node`:
//! atoms (`|-`, `:`) interleaved with argument holes.
//! As a tree it is a `Mixfix`, and `Mixop` is the form without arguments.
//! A `ShapeKind` is the same form interned node by node in a `ShapeArena`,
//! so a case value keeps a `Shape` handle apart from its arguments;
//! `MixopShape` pairs a prepared notation with its shape.
//! Shape numbers never leave the arena: payloads write the notation out.

mod arena;
pub mod mixop;
mod mixop_shape;
mod node;
mod shape;
mod tree;

pub use arena::{ShapeArena, ShapeError};
pub use mixop::ArityMismatch;
pub use mixop_shape::MixopShape;
pub use node::{AtomPhrase, Node, Repr};
pub use shape::{Shape, ShapeKind};
pub use tree::{Mixfix, Mixop, Tree};
