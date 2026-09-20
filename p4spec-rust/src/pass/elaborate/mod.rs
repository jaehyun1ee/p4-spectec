//! Surface-language validation and conversion to the intermediate language
//!
//! `convert` elaborates an EL specification into a typed IL specification.
//!
//! Definitions are checked in source order,
//! rule groups and clauses are attached to their relation or function,
//! and iterations are annotated with the variables they range over.
//!
//! For example, `-- if (n_x = n_y)*` becomes `-- if (n_x = n_y)*{n_x <- n_x*}`
//! once the dimensions of `n_x` and `n_y` are known.

#![allow(clippy::result_large_err)]

mod attempt;
mod context;
mod dimension;
mod elab;
mod error;

pub use error::*;

use crate::lang::{el, il};

// == Entry point

/// Validates and converts an EL specification to IL.
pub fn convert(spec_el: el::ast::Spec) -> Result<il::ast::Spec, ElabError> {
    elab::elaborate(spec_el)
}
