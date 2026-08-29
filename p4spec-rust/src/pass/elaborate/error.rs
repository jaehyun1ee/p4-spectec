//! Typed failures produced while elaborating surface-language syntax

use thiserror::Error;

use crate::{
    lang::common::source::Span,
    runtime::types::{TypeError, TypeErrorKind},
};

/// Namespace or definition family involved in a lookup failure
#[derive(Clone, Copy, Debug, Error, PartialEq, Eq)]
pub enum EntityKind {
    #[error("type")]
    Type,
    #[error("meta-variable")]
    MetaVariable,
    #[error("relation")]
    Relation,
    #[error("defined relation")]
    DefinedRelation,
    #[error("rule group")]
    RuleGroup,
    #[error("function")]
    Function,
    #[error("defined function")]
    DefinedFunction,
    #[error("table function")]
    TableFunction,
    #[error("otherwise group")]
    ElseGroup,
    #[error("otherwise clause")]
    ElseClause,
}

/// Structural type expected by a failed elaboration alternative
#[derive(Clone, Copy, Debug, Error, PartialEq, Eq)]
pub enum TypeShape {
    #[error("text")]
    Text,
    #[error("iteration")]
    Iteration,
    #[error("tuple")]
    Tuple,
    #[error("list")]
    List,
    #[error("struct")]
    Struct,
}

/// Stable semantic category of an elaboration failure
#[derive(Clone, Debug, Error, PartialEq, Eq)]
pub enum ElabErrorKind {
    #[error("undefined {0}")]
    Undefined(EntityKind),
    #[error("duplicate {0}")]
    Duplicate(EntityKind),
    #[error("type operation failed: {0}")]
    Type(TypeErrorKind),
    #[error("cannot destruct type as {0}")]
    CannotDestructure(TypeShape),
    #[error("cannot infer expression type")]
    CannotInfer,
    #[error("operator is not defined for the operand types")]
    OperatorNotDefined,
    #[error("types do not match")]
    TypeMismatch,
    #[error("iteration dimensions do not match")]
    DimensionMismatch,
    #[error("invalid or empty iteration")]
    InvalidIteration,
    #[error("construct is not valid at this elaboration stage")]
    MisplacedConstruct,
    #[error("argument or parameter arity does not match")]
    ArityMismatch,
    #[error("identifier is invalid in this role")]
    InvalidIdentifier,
    #[error("variant cases are ambiguous")]
    AmbiguousVariant,
    #[error("type extension target is invalid")]
    InvalidTypeExtension,
    #[error("expression cannot be cast to the expected type")]
    InvalidCast,
    #[error("argument does not match its parameter")]
    InvalidArgument,
    #[error("premise is invalid")]
    InvalidPremise,
    #[error("rule is invalid")]
    InvalidRule,
    #[error("definition is invalid")]
    InvalidDefinition,
    #[error("input hint is invalid")]
    InvalidInputHint,
    #[error("definition was already populated")]
    AlreadyPopulated,
    #[error("no elaboration alternative matched")]
    NoMatchingAlternative,
}

/// An elaboration failure paired with the source span that caused it
#[derive(Clone, Debug, Error, PartialEq, Eq)]
#[error("{diagnostic} at {span}")]
pub struct ElabError {
    pub kind: ElabErrorKind,
    pub span: Span,
    diagnostic: String,
}

impl ElabError {
    pub(crate) fn new(kind: ElabErrorKind, span: Span, diagnostic: impl Into<String>) -> Self {
        Self {
            kind,
            span,
            diagnostic: diagnostic.into(),
        }
    }

    pub(crate) fn undefined(entity: EntityKind, name: &str, span: Span) -> Self {
        Self::new(
            ElabErrorKind::Undefined(entity),
            span,
            format!("{entity} `{name}` is undefined"),
        )
    }

    pub(crate) fn duplicate(entity: EntityKind, name: &str, span: Span) -> Self {
        Self::new(
            ElabErrorKind::Duplicate(entity),
            span,
            format!("{entity} `{name}` was already defined"),
        )
    }
}

impl From<TypeError> for ElabError {
    fn from(error: TypeError) -> Self {
        let diagnostic = error.kind.to_string();
        Self::new(ElabErrorKind::Type(error.kind), error.span, diagnostic)
    }
}
