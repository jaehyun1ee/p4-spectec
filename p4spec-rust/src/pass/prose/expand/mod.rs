//! Lift nested calls into explicit SL let instructions

#[allow(clippy::module_inception)]
mod expand;
mod lift;

use crate::lang::sl::ast as sl;

use super::ProseError;

/// Lifts nested calls in a structured-language specification.
pub(super) fn expand_spec(spec_sl: sl::Spec) -> Result<sl::Spec, ProseError> {
    expand::expand_spec(spec_sl)
}
