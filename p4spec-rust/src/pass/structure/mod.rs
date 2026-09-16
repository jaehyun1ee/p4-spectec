//! Structure AL definitions into SL through pass-local ordered instructions

mod antiunify;
mod context;
mod dangle;
mod error;
mod ol;
mod opt;
mod pretty;
mod re;
mod totalize;
mod transform;

pub use error::{StructureError, StructureErrorKind};

use crate::lang::{al::ast as al, sl::ast as sl};

#[cfg(test)]
#[path = "../../../tests/pass/structure/internal.rs"]
mod tests;

// == Entry point

/// Converts algorithmic definitions, removing rule groups when requested
pub fn convert(spec_al: al::Spec, without_rule_groups: bool) -> Result<sl::Spec, StructureError> {
    transform::r#struct(spec_al, without_rule_groups)
}
