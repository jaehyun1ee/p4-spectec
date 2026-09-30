//! Interned notation shapes with exact locations and canonical identities
//!
//! A case value `LEFT n` keeps the shape of `LEFT _` apart from its argument.
//! `ShapeKind` is one notation node whose children are `Shape` handles,
//! so shapes are interned children first and stored without recursion.
//! Exact identity keeps atom spans;
//! canonical identity compares atom names and children's canonical ids.
//! `ShapeArena::intern_notation` interns a `Mixfix` and counts its arguments.

mod arena;
mod kind;

pub use arena::ShapeArena;
pub use kind::{Shape, ShapeKind};
