pub(crate) mod context;
mod error;
mod ol;

pub use error::{StructureError, StructureErrorKind};
#[cfg(test)]
#[path = "../../../tests/pass/structure/internal.rs"]
mod tests;
