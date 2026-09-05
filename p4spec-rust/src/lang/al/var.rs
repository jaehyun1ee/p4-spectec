//! Variables for algorithmic language data

use super::ast::*;

// Variable with type and dimension

pub type T = Var;

// Conversion to expression

pub fn as_exp(var: &T, dim: bool) -> Exp {
    crate::lang::il::var::as_exp(var, dim)
}
