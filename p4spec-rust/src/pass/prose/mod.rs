//! Convert structured-language definitions to prose-language definitions

mod context;
mod error;
mod expand;
mod prosify;
mod shorthand;
mod stamp;

pub use error::{ProseError, ProseErrorKind};

use context::Context;

use crate::lang::{pl::ast as pl, sl::ast as sl};

/// Converts a rule-group-preserving SL specification to PL
pub fn convert(spec_sl: sl::Spec) -> Result<pl::Spec, ProseError> {
    prosify::prosify(spec_sl)
}
