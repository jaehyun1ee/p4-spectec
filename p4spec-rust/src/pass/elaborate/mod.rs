//! Surface-language validation and conversion to the intermediate language

mod attempt;
mod context;
mod dimension;
mod elab;
mod error;

pub use error::*;

use crate::lang::{el, il};

/// Validates and converts an elaboration-language specification to intermediate syntax.
pub fn elaborate(spec: &el::ast::Spec) -> Result<il::ast::Spec, ElabError> {
    elab::elaborate(spec)
}
