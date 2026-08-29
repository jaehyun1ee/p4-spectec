//! Intermediate-to-algorithmic language conversion

mod attempt;
mod error;

pub mod binding;

pub use error::*;

use crate::lang::{al, il};

/// Converts an intermediate-language specification to algorithmic syntax.
pub fn convert(_spec: &il::ast::Spec) -> Result<al::ast::Spec, AlgoError> {
    attempt::fail(AlgoError::new(
        AlgoErrorKind::Unsupported,
        Default::default(),
    ))
}
