//! Intermediate-to-algorithmic language conversion

mod attempt;
mod error;

pub mod binding;

pub use error::*;

use crate::lang::{al, il};

/// Converts an intermediate-language specification to algorithmic syntax.
pub fn convert(spec: &il::ast::Spec) -> Result<al::ast::Spec, AlgoError> {
    let span = spec.first().map(|def| def.span.clone()).unwrap_or_default();
    attempt::fail(AlgoError::new(AlgoErrorKind::Unsupported, span))
}
