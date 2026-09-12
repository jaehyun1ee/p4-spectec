mod antiunify;
mod context;
mod dangle;
mod error;
mod merge;
mod ol;
mod opt;
mod optimize;
mod prettify;
mod pretty;
mod re;
mod totalize;
mod transform;

pub use error::{StructureError, StructureErrorKind};
pub use transform::convert;

#[cfg(test)]
#[path = "../../../tests/pass/structure/internal.rs"]
mod tests;
