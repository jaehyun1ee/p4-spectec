//! Structure AL definitions into SL through pass-local ordered instructions

mod antiunify;
mod context;
mod dangle;
mod error;
mod merge;
mod ol;
mod opt;
mod pretty;
mod re;
mod totalize;
mod transform;

pub use error::{StructureError, StructureErrorKind};
pub use transform::convert;

#[cfg(test)]
#[path = "../../../tests/pass/structure/internal.rs"]
mod tests;
