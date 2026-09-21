//! Surface-language validation and conversion to the intermediate language

#![allow(clippy::result_large_err)]

mod attempt;
mod context;
mod dimension;
mod error;
mod transform;

pub use error::*;

use crate::lang::{el, il};

// == Entry point

/// Validates and converts an EL specification to IL
pub fn convert(spec_el: el::ast::Spec) -> Result<il::ast::Spec, ElabError> {
    transform::elab_spec(spec_el)
}
