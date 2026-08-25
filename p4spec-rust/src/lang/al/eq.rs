//! Semantic equality for algorithmic-language data
//!
//! Ignores source regions;
//! delegates shared syntax to IL equality

// Identifiers

pub use crate::lang::il::eq::eq_id;

// Atoms

pub use crate::lang::il::eq::{eq_atom, eq_atoms};

// Mixfix operators

pub use crate::lang::il::eq::eq_mixop;

// Iterators

pub use crate::lang::il::eq::{eq_iter, eq_iters};

// Variables

pub use crate::lang::il::eq::{eq_var, eq_vars};

// Types

pub use crate::lang::il::eq::{eq_nottyp, eq_typ, eq_typs};

// Values

pub use crate::lang::il::eq::{eq_value, eq_values};

// Expressions

pub use crate::lang::il::eq::{eq_exp, eq_exps, eq_iterexp, eq_iterexps};

// Patterns

pub use crate::lang::il::eq::eq_pattern;

// Paths

pub use crate::lang::il::eq::eq_path;

// Type parameters

pub use crate::lang::il::eq::{eq_tparam, eq_tparams};

// Arguments

pub use crate::lang::il::eq::{eq_arg, eq_args};

// Type arguments

pub use crate::lang::il::eq::{eq_targ, eq_targs};

// Premises

pub use crate::lang::il::eq::{eq_iterprem, eq_iterprems, eq_prem};
