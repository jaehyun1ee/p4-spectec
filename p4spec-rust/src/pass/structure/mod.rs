mod context;
mod error;
mod antiunify;
mod ol;
mod opt;
mod merge;
mod re;

pub use error::{StructureError, StructureErrorKind};
#[cfg(test)]
#[path = "../../../tests/pass/structure/internal.rs"]
mod tests;

mod pretty;
mod prettify;

mod totalize;
mod dangle;

mod optimize;
