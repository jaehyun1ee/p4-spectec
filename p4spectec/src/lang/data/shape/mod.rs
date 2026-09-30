//! Interned notation shapes with exact locations and canonical identities
//!
//! A case value `LEFT n` keeps the shape of `LEFT _` apart from its argument.
//! `ShapeKind` is one notation node whose children are `Shape` handles;
//! exact identity keeps atom spans,
//! canonical identity compares atom names and children's canonical ids.
//! `ShapeArena::intern_notation` interns a `Mixfix` and counts its arguments;
//! `MixopShape` pairs a prepared notation with its shape.
//! Shape numbers never leave the arena: payloads write the notation out.

mod arena;
mod kind;
mod mixop;

pub use arena::ShapeArena;
pub use kind::{Shape, ShapeKind};
pub use mixop::MixopShape;
