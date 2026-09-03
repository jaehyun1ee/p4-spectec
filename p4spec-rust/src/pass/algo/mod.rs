//! Intermediate-to-algorithmic language conversion

mod error;
mod sidecondition;

#[cfg(test)]
#[path = "../../../tests/pass/algo/internal.rs"]
mod tests;

mod binding;

pub use error::*;

use crate::lang::{al, il};

/// Converts an IL specification to AL
pub fn convert(spec_il: il::ast::Spec) -> Result<al::ast::Spec, AlgoError> {
    let spec_al = binding::analyze::analyze_spec(spec_il)?;
    Ok(sidecondition::guard::insert_spec(spec_al))
}
