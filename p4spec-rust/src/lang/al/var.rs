//! Algorithmic-language variable expressions

use super::ast::*;

// == Variables

pub type Variable = Var;

// - Conversion to expressions

/// Converts a variable to an expression
pub fn as_exp(is_dim: bool, var: &Variable) -> Exp {
    crate::lang::il::var::as_exp(is_dim, var)
}
