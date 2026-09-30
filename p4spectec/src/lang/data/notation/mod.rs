//! Notation of IL and later stages, as trees and as interned shapes
//!
//! A notation form such as `C |- e : t` is a `Mixfix`:
//! atoms (`|-`, `:`) interleaved with argument holes;
//! `Mixop` is the form without arguments.
//! A `ShapeKind` is the same form interned node by node in a `ShapeArena`,
//! so a case value keeps a `Shape` handle apart from its arguments;
//! `MixopShape` pairs a prepared notation with its shape.
//! Shape numbers never leave the arena: payloads write the notation out.

mod arena;
mod mixfix;
pub mod mixop;
mod mixop_shape;
mod shape;

pub use arena::{ShapeArena, ShapeError};
pub use mixfix::{AtomPhrase, Mixfix};
pub use mixop::{ArityMismatch, Mixop};
pub use mixop_shape::MixopShape;
pub use shape::{Shape, ShapeKind};
