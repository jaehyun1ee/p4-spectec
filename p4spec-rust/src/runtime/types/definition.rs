use thiserror::Error;

use crate::lang::{
    common::{ds::map::IdMap, source::Span},
    il::ast::{DefTyp, TParam},
};

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

/// Type definitions keyed by source-insensitive type identifiers
pub type TypeEnvironment = IdMap<TypeDefinition>;

/// A failure in a runtime type operation
#[derive(Clone, Debug, Error, PartialEq, Eq)]
#[error("{kind} at {span}")]
pub struct TypeError {
    pub kind: TypeErrorKind,
    pub span: Span,
}

impl TypeError {
    pub(crate) fn new(kind: TypeErrorKind, span: Span) -> Self {
        Self { kind, span }
    }
}

/// Category of a runtime type failure
#[derive(Clone, Debug, Error, PartialEq, Eq)]
pub enum TypeErrorKind {
    #[error("higher-order substitution is disallowed")]
    HigherOrderSubstitution,

    #[error("type argument count differs: expected {expected}, got {actual}")]
    TypeArgumentCount { expected: usize, actual: usize },

    #[error("type variable {0} is not defined")]
    UndefinedType(String),

    #[error("type parameter counts differ: {left} and {right}")]
    TypeParameterCount { left: usize, right: usize },

    #[error("parameter counts differ: {left} and {right}")]
    ParameterCount { left: usize, right: usize },
}
