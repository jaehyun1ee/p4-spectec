mod context;
mod error;
mod antiunify;
mod ol;
mod merge;
mod re;

pub use error::{StructureError, StructureErrorKind};
#[cfg(test)]
#[path = "../../../tests/pass/structure/internal.rs"]
mod tests;
