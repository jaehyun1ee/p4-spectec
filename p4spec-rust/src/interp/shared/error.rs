//! Located interpreter lookup, host, and execution failures
//!
//! An `Error` is a kind, a span, and the child failures that led to it.
//! `ErrorKind` groups kinds by origin:
//! context lookups, runtime value operations, host interfaces,
//! and each evaluation stage.
//! `Display` prints the trace as a tree and folds runs deeper than ten levels.

use crate::runner::{ExternError, InterfaceError};
use num_bigint::BigInt;
use std::fmt;

use crate::{
    lang::{
        common::prim::num::NumericError,
        common::{
            ds::map::ArityMismatch, notation::mixop::ArityMismatch as MixopArityMismatch,
            source::Span,
        },
        data::value::ValueError,
        hints::input::InputError,
    },
    runtime::ops::{
        typ::{TypeError, TypeErrorKind},
        value::MatchError,
    },
};

/// Namespace of a failed context lookup.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EntityKind {
    Value,
    Type,
    DefinedType,
    Relation,
    Function,
}

impl fmt::Display for EntityKind {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::Value => "value",
            Self::Type => "type",
            Self::DefinedType => "defined type",
            Self::Relation => "relation",
            Self::Function => "function",
        })
    }
}

/// Origin category of a failure.
#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
pub enum ErrorKind {
    #[error(transparent)]
    /// Binding lookups.
    Context(#[from] ContextErrorKind),
    #[error(transparent)]
    /// Value and type operations.
    Runtime(#[from] RuntimeErrorKind),
    #[error(transparent)]
    /// Interface and extern calls.
    Host(#[from] HostErrorKind),
    #[error(transparent)]
    /// Destructuring assignment.
    Assign(#[from] AssignErrorKind),
    #[error(transparent)]
    /// Expression evaluation.
    Expr(#[from] ExprErrorKind),
    #[error(transparent)]
    /// Premise checks.
    Prem(#[from] PremErrorKind),
    #[error(transparent)]
    /// Function and relation invocation.
    Call(#[from] CallErrorKind),
    #[error(transparent)]
    /// Type guards at call boundaries.
    Guard(#[from] GuardErrorKind),
    #[error(transparent)]
    /// Trace nodes grouping child failures.
    Trace(#[from] TraceErrorKind),
}

/// Failures of context lookups and iterated bindings.
#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
pub enum ContextErrorKind {
    #[error("{kind} `{name}` is undefined")]
    Undefined { kind: EntityKind, name: String },
    #[error("{kind} `{name}` was already defined")]
    Duplicate { kind: EntityKind, name: String },
    #[error("mismatch in optionality of iterated variables")]
    OptionalityMismatch,
    #[error("cannot transpose a matrix of value batches")]
    IterationLengthMismatch { expected: usize, actual: usize },
}

/// Failures lifted from value, numeric, type, and matching operations.
#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
pub enum RuntimeErrorKind {
    #[error(transparent)]
    Value(#[from] ValueError),
    #[error(transparent)]
    Numeric(#[from] NumericError),
    #[error(transparent)]
    Type(TypeErrorKind),
    #[error(transparent)]
    Input(#[from] InputError),
    #[error(transparent)]
    Arity(#[from] ArityMismatch),
    #[error(transparent)]
    MixopArity(#[from] MixopArityMismatch),
    #[error("{}", MatchDisplay(.0))]
    Match(MatchError),
}

/// Failures reported by the host interface or externs.
#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
pub enum HostErrorKind {
    #[error(transparent)]
    Interface(#[from] InterfaceError),
    #[error(transparent)]
    Extern(#[from] ExternError),
}

/// Failures of destructuring assignment.
#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
pub enum AssignErrorKind {
    #[error("match failed {exp} <- {value}")]
    Mismatch { exp: String, value: String },
    #[error(
        "mismatch in number of expressions and values while assigning, expected {expected} value(s) but got {actual}"
    )]
    ExpressionArityMismatch { expected: usize, actual: usize },
    #[error(
        "mismatch in number of arguments and values while assigning, expected {expected} value(s) but got {actual}"
    )]
    ArgumentArityMismatch { expected: usize, actual: usize },
    #[error("cannot assign an empty list to a cons expression")]
    EmptyCons,
    #[error("cannot assign a value {value} to a definition {def}")]
    DefinitionMismatch { value: String, def: String },
}

/// Failures of built-in expression operators.
#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
pub enum ExprErrorKind {
    #[error("concatenation expects either two texts or two lists")]
    ConcatenationOperandMismatch,
    #[error("length operation expects either a text or a list")]
    LengthOperandMismatch,
    #[error("undefined structure field")]
    UndefinedField,
    #[error("text byte slice is not on UTF-8 boundaries")]
    TextSliceBoundaryMismatch,
    #[error("indexing expects either a text or a list")]
    IndexOperandMismatch,
    #[error("slicing expects either a text or a list")]
    SliceOperandMismatch,
    #[error("updating a character requires a single-character text")]
    CharacterUpdateLengthMismatch,
    #[error("tuple cast arity mismatch")]
    TupleCastArityMismatch { expected: usize, actual: usize },
    #[error("index {idx} out of bounds [0, {len})")]
    IndexOutOfBounds { idx: BigInt, len: usize },
    #[error("slice [{idx}, {end}) out of bounds [0, {size})")]
    SliceOutOfBounds { idx: BigInt, end: BigInt, size: usize },
    #[error(
        "updating a slice of length {len} requires a text of length {len}, but got length {actual}"
    )]
    TextSliceUpdateLengthMismatch { len: usize, actual: usize },
    #[error(
        "updating a slice of length {len} requires a list of length {len}, but got length {actual}"
    )]
    ListSliceUpdateLengthMismatch { len: usize, actual: usize },
}

/// Premises that did not hold.
#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
pub enum PremErrorKind {
    #[error("condition {exp} was not met")]
    ConditionNotMet { exp: String },
    #[error("condition hold {relation} was not met")]
    HoldConditionNotMet { relation: String },
    #[error("condition not-hold {relation} was not met")]
    NotHoldConditionNotMet { relation: String },
}

/// Type guards violated at call boundaries.
#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
pub enum GuardErrorKind {
    #[error("relation input of {relation} does not match the expected type")]
    RelationInputMismatch { relation: String },
    #[error("relation output of {relation} does not match the expected type")]
    RelationOutputMismatch { relation: String },
    #[error("function argument of {func} does not match the parameter type")]
    FunctionInputMismatch { func: String },
    #[error("return value of function {func} does not match the expected type")]
    FunctionOutputMismatch { func: String },
}

/// Failures of invocation: arity, flow, and nondeterminism.
#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
pub enum CallErrorKind {
    #[error("nondeterministic instruction evaluation")]
    InstructionNondeterminism,
    #[error("{message}")]
    InvalidFlow { message: &'static str },
    #[error("arity mismatch in rule")]
    RuleArityMismatch { expected: usize, actual: usize },
    #[error("arity mismatch while matching table row")]
    TableRowArityMismatch { expected: usize, actual: usize },
    #[error("arity mismatch in type arguments")]
    TypeArgumentArityMismatch { expected: usize, actual: usize },
    #[error("arity mismatch while matching clause")]
    ClauseArityMismatch { expected: usize, actual: usize },
    #[error(
        "non-deterministic application of relation {relation}: {group_a}/{path_a}, {group_b}/{path_b}"
    )]
    RelationNondeterminism {
        relation: String,
        group_a: String,
        path_a: String,
        group_b: String,
        path_b: String,
    },
    #[error("non-deterministic application of function {func}: {first}, {second}")]
    FunctionNondeterminism { func: String, first: usize, second: usize },
}

/// Grouping nodes in a failure trace.
#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
pub enum TraceErrorKind {
    #[error("execution failed")]
    Execution,
    #[error("invocation of {text} failed")]
    Invocation { text: String },
    #[error("evaluation of {text} failed")]
    Evaluation { text: String },
}

impl TraceErrorKind {
    /// Names an invocation trace `$f<T>` for function `id` with `targs`.
    pub(crate) fn function(
        id: &crate::lang::il::ast::Id,
        targs: &[crate::lang::il::ast::Typ],
    ) -> Self {
        TraceErrorKind::Invocation {
            // Show type arguments only when present
            text: if targs.is_empty() {
                format!("${}", id.node)
            } else {
                format!(
                    "${}<{}>",
                    id.node,
                    targs
                        .iter()
                        .map(crate::lang::traits::print::Print::to_string)
                        .collect::<Vec<_>>()
                        .join(", ")
                )
            },
        }
    }
}

/// A located failure with the child failures that led to it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Error {
    pub kind: Box<ErrorKind>,
    pub span: Span,
    /// Failures of the alternatives or sub-steps that led here.
    pub children: Vec<Error>,
}

impl Error {
    /// Height of the trace tree.
    pub(crate) fn depth(&self) -> usize {
        // Iterative walk: deep traces would overflow a recursive one
        let mut depth = 0;
        let mut pending = vec![(self, 1)];
        while let Some((error, depth_error)) = pending.pop() {
            depth = depth.max(depth_error);
            pending.extend(error.children.iter().map(|error| (error, depth_error + 1)));
        }
        depth
    }

    pub fn new(kind: ErrorKind, span: Span) -> Self {
        Self { kind: Box::new(kind), span, children: Vec::new() }
    }

    /// An unlocated root grouping the traces of a failed execution.
    pub fn execution(children: Vec<Error>) -> Self {
        Self {
            kind: Box::new(ErrorKind::Trace(TraceErrorKind::Execution)),
            span: Span::default(),
            children,
        }
    }

    /// Locates an unlocated error at `span`; guard errors keep their own.
    pub fn at_if_missing(mut self, span: &Span) -> Self {
        if self.span == Span::default() && !matches!(*self.kind, ErrorKind::Guard(_)) {
            self.span = span.clone();
        }
        self
    }

    /// An undefined-entity lookup failure.
    pub(crate) fn undefined(kind: EntityKind, name: String, span: Span) -> Self {
        Self::new(ErrorKind::Context(ContextErrorKind::Undefined { kind, name }), span)
    }

    /// A duplicate-definition failure.
    pub(crate) fn duplicate(kind: EntityKind, name: String, span: Span) -> Self {
        Self::new(ErrorKind::Context(ContextErrorKind::Duplicate { kind, name }), span)
    }
}

impl From<ErrorKind> for Error {
    fn from(kind: ErrorKind) -> Self {
        Self::new(kind, Span::default())
    }
}

/// Lifts a host or runtime error type into the matching `ErrorKind` variant.
macro_rules! from_error {
    ($error:ty, $category:ident, $kind:ident, $variant:ident) => {
        impl From<$error> for Error {
            fn from(error: $error) -> Self {
                Self::new(ErrorKind::$category($kind::$variant(error)), Span::default())
            }
        }
    };
}
from_error!(InterfaceError, Host, HostErrorKind, Interface);
from_error!(ExternError, Host, HostErrorKind, Extern);
from_error!(ValueError, Runtime, RuntimeErrorKind, Value);
from_error!(NumericError, Runtime, RuntimeErrorKind, Numeric);
from_error!(InputError, Runtime, RuntimeErrorKind, Input);
from_error!(ArityMismatch, Runtime, RuntimeErrorKind, Arity);
from_error!(MixopArityMismatch, Runtime, RuntimeErrorKind, MixopArity);

impl From<TypeError> for Error {
    fn from(error: TypeError) -> Self {
        Self::new(ErrorKind::Runtime(RuntimeErrorKind::Type(error.kind)), error.span)
    }
}

impl From<MatchError> for Error {
    fn from(error: MatchError) -> Self {
        let span = match &error {
            MatchError::UndefinedType { span, .. }
            | MatchError::UnexpectedTypeVariable { span }
            | MatchError::TypeArgumentMismatch { span, .. }
            | MatchError::UndefinedFunction { span, .. } => span.clone(),
            MatchError::Type(error) => error.span.clone(),
        };
        Self::new(ErrorKind::Runtime(RuntimeErrorKind::Match(error)), span)
    }
}

/// Prints a `MatchError` without its spans.
struct MatchDisplay<'a>(&'a MatchError);
impl fmt::Display for MatchDisplay<'_> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.0 {
            MatchError::UndefinedType { name, .. } => write!(formatter, "undefined type {name}"),
            MatchError::UnexpectedTypeVariable { .. } => {
                formatter.write_str("unexpected type variable")
            }
            MatchError::TypeArgumentMismatch { expected, actual, .. } => {
                write!(formatter, "expected {expected} type arguments, got {actual}")
            }
            MatchError::UndefinedFunction { name, .. } => {
                write!(formatter, "undefined function {name}")
            }
            MatchError::Type(error) => fmt::Display::fmt(&error.kind, formatter),
        }
    }
}

impl std::error::Error for Error {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        Some(self.kind.as_ref())
    }
}

impl fmt::Display for Error {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        // An execution root prints its children directly
        if matches!(*self.kind, ErrorKind::Trace(TraceErrorKind::Execution)) {
            fmt::Display::fmt(&TraceDisplay(&self.children), formatter)
        } else {
            fmt::Display::fmt(&TraceDisplay(std::slice::from_ref(self)), formatter)
        }
    }
}

/// Prints a trace list as a tree, folding subtrees deeper than ten levels.
struct TraceDisplay<'a>(&'a [Error]);
impl fmt::Display for TraceDisplay<'_> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        // First pass: height of every subtree
        let mut depths = std::collections::HashMap::<*const Error, usize>::new();
        let mut pending = self
            .0
            .iter()
            .map(|error| (error, false))
            .collect::<Vec<_>>();
        while let Some((error, visited)) = pending.pop() {
            if visited {
                let depth = 1 + error
                    .children
                    .iter()
                    .map(|error| depths[&(error as *const Error)])
                    .max()
                    .unwrap_or(0);
                depths.insert(error as *const Error, depth);
            } else {
                pending.push((error, true));
                pending.extend(error.children.iter().map(|error| (error, false)));
            }
        }
        // Second pass: print depth-first, folding tall subtrees into a count
        let mut pending = self
            .0
            .iter()
            .enumerate()
            .rev()
            .map(|(idx, error)| (error, idx, self.0.len(), String::new(), 0, true))
            .collect::<Vec<_>>();
        while let Some((error, idx, len, indent, run, root)) = pending.pop() {
            // Skip this level, counting it as an omitted trace
            if !root && depths[&(error as *const Error)] > 10 {
                pending.extend(
                    error
                        .children
                        .iter()
                        .enumerate()
                        .rev()
                        .map(|(idx, error_sub)| {
                            (error_sub, idx, error.children.len(), indent.clone(), run + 1, false)
                        }),
                );
                continue;
            }
            // Report the omitted run before the next printed node
            if run > 0 {
                writeln!(formatter, "{indent}│ ··· omitting {run} traces ···")?;
            }
            let last = idx + 1 == len;
            let boundary = root || run > 0;
            write!(formatter, "{indent}")?;
            if !boundary {
                write!(formatter, "{}", if last { "└── " } else { "├── " })?;
            }
            if error.span != Span::default() {
                writeln!(formatter, "{}", error.span)?;
                write!(formatter, "{indent}{}", if boundary { "" } else { "    " })?;
            }
            if len > 1 {
                write!(formatter, "{}. ", idx + 1)?;
            }
            writeln!(formatter, "{}", error.kind)?;
            // Children indent under this node
            let indent_sub = if boundary {
                indent
            } else {
                format!("{indent}{}", if last { "    " } else { "│   " })
            };
            pending.extend(
                error
                    .children
                    .iter()
                    .enumerate()
                    .rev()
                    .map(|(idx, error_sub)| {
                        (error_sub, idx, error.children.len(), indent_sub.clone(), 0, false)
                    }),
            );
        }
        Ok(())
    }
}

impl Drop for Error {
    fn drop(&mut self) {
        // Flatten the tree iteratively so deep traces do not overflow the stack
        let mut pending = std::mem::take(&mut self.children);
        while let Some(mut error) = pending.pop() {
            pending.append(&mut error.children);
        }
    }
}
