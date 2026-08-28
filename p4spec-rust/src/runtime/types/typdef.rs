use crate::lang::il::ast::{DefTyp, TParam};

/// State of a type identifier in the static type environment
#[derive(Clone, Debug, PartialEq)]
pub enum TypeDefinition {
    /// A locally bound type parameter
    Parameter,
    /// An externally supplied type
    Extern,
    /// A type whose declaration is currently being checked
    Defining(Vec<TParam>),
    /// A fully checked type declaration
    Defined(Vec<TParam>, Box<DefTyp>),
}

impl TypeDefinition {
    /// Returns the declaration's type parameters
    pub fn parameters(&self) -> &[TParam] {
        match self {
            Self::Parameter | Self::Extern => &[],
            Self::Defining(parameters) | Self::Defined(parameters, _) => parameters,
        }
    }
}
