//! Lift nested calls into explicit SL let instructions

mod lift;
mod transform;

use crate::lang::sl::ast as sl;

use super::ProseError;

/// Lifts nested calls in a structured-language specification.
pub(super) fn expand_spec(spec_sl: sl::Spec) -> Result<sl::Spec, ProseError> {
    transform::expand_spec(spec_sl)
}
