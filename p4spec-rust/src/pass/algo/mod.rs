//! Intermediate-to-algorithmic language conversion

mod attempt;
mod error;
mod sidecondition;

pub mod binding;

pub use error::*;

use crate::lang::{al, il};

/// Converts an intermediate-language specification to algorithmic syntax.
///
/// Binding analysis is an implementation detail; callers must use this entry point so
/// side-condition guards cannot be skipped.
///
/// ```compile_fail
/// use p4spec_rust::{lang::il, pass::algo::binding};
///
/// let spec: il::ast::Spec = vec![];
/// let _unguarded = binding::analyze::analyze_spec(&spec);
/// ```
pub fn convert(spec: &il::ast::Spec) -> Result<al::ast::Spec, AlgoError> {
    let analyzed: attempt::Attempt<_> = binding::analyze::analyze_spec(spec);
    match analyzed {
        Ok(spec) => Ok(sidecondition::guard::insert_spec(spec)),
        Err(error) => attempt::fail(error),
    }
}
