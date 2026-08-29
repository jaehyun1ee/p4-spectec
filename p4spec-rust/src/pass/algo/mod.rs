//! Intermediate-to-algorithmic language conversion

mod attempt;
mod error;
mod sidecondition;

pub mod binding;

pub use error::*;

use crate::lang::{al, il};

/// Converts an intermediate-language specification to algorithmic syntax.
pub fn convert(spec: &il::ast::Spec) -> Result<al::ast::Spec, AlgoError> {
    let analyzed: attempt::Attempt<_> = binding::analyze::analyze_spec(spec);
    match analyzed {
        Ok(spec) => Ok(sidecondition::guard::insert_spec(spec)),
        Err(error) => attempt::fail(error),
    }
}
