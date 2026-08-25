use super::ast::*;

// Variable with type and dimention

pub type Variable = Var;

// Conversion to expression

pub fn as_exp(var: &Variable, dim: bool) -> Exp {
    crate::lang::il::var::as_exp(var, dim)
}
