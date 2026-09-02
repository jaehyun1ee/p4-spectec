//! Surface-language validation and conversion to the intermediate language

#![allow(clippy::result_large_err)]

mod attempt;
mod context;
mod dimension;
mod elab;
mod error;

pub use error::*;

use crate::lang::{el, il};

/// Validates and converts an EL specification to IL
pub fn elaborate(spec_el: el::ast::Spec) -> Result<il::ast::Spec, ElabError> {
    elab::elaborate(spec_el)
}
