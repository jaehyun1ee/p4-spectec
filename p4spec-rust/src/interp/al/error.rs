//! Located AL lookup, host, and execution failures

use crate::runner::{ExternError, InterfaceError};
use std::fmt;

use crate::{
    lang::{
        common::{
            ds::map::ArityMismatch, notation::mixop::ArityMismatch as MixopArityMismatch,
            source::Span,
        },
        data::value::ValueError,
        hints::input::InputError,
        xl::num::NumericError,
    },
    runtime::ops::{
        typ::{TypeError, TypeErrorKind},
        value::MatchError,
    },
};

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

#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
pub enum ErrorKind {
    #[error(transparent)]
    Context(#[from] ContextErrorKind),
    #[error(transparent)]
    Runtime(#[from] RuntimeErrorKind),
    #[error(transparent)]
    Host(#[from] HostErrorKind),
    #[error(transparent)]
    Assign(#[from] AssignErrorKind),
    #[error(transparent)]
    Expr(#[from] ExprErrorKind),
    #[error(transparent)]
    Prem(#[from] PremErrorKind),
    #[error(transparent)]
    Call(#[from] CallErrorKind),
    #[error(transparent)]
    Guard(#[from] GuardErrorKind),
    #[error(transparent)]
    Trace(#[from] TraceErrorKind),
}

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

#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
pub enum HostErrorKind {
    #[error(transparent)]
    Interface(#[from] InterfaceError),
    #[error(transparent)]
    Extern(#[from] ExternError),
}

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

#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
pub enum ExprErrorKind {
    #[error("concatenation expects either two texts or two lists")]
    ConcatenationOperandMismatch,
    #[error("length operation expects either a text or a list")]
    LengthOperandMismatch,
    #[error("index does not fit a machine integer")]
    IndexOverflow,
    #[error("undefined structure field")]
    UndefinedField,
    #[error("text byte slice is not on UTF-8 boundaries")]
    TextSliceBoundaryMismatch,
    #[error("indexing expects either a text or a list")]
    IndexOperandMismatch,
    #[error("slice end overflows a machine integer")]
    SliceEndOverflow,
    #[error("slicing expects either a text or a list")]
    SliceOperandMismatch,
    #[error("text slice length is negative")]
    NegativeTextSliceLength,
    #[error("updating a character requires a single-character text")]
    CharacterUpdateLengthMismatch,
    #[error("tuple cast arity mismatch")]
    TupleCastArityMismatch { expected: usize, actual: usize },
    #[error("index {idx} out of bounds [0, {len})")]
    IndexOutOfBounds { idx: i64, len: usize },
    #[error("slice [{idx}, {end}) out of bounds [0, {size})")]
    SliceOutOfBounds { idx: i64, end: i64, size: usize },
    #[error(
        "updating a slice of length {len} requires a text of length {len}, but got length {actual}"
    )]
    TextSliceUpdateLengthMismatch { len: i64, actual: usize },
    #[error(
        "updating a slice of length {len} requires a list of length {len}, but got length {actual}"
    )]
    ListSliceUpdateLengthMismatch { len: i64, actual: usize },
}

#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
pub enum PremErrorKind {
    #[error("condition {exp} was not met")]
    ConditionNotMet { exp: String },
    #[error("condition hold {relation} was not met")]
    HoldConditionNotMet { relation: String },
    #[error("condition not-hold {relation} was not met")]
    NotHoldConditionNotMet { relation: String },
}

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
    #[error(transparent)]
    Validation(Box<ErrorKind>),
}

#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
pub enum CallErrorKind {
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
    FunctionNondeterminism {
        func: String,
        first: usize,
        second: usize,
    },
}

#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
pub enum TraceErrorKind {
    #[error("execution failed")]
    Execution,
    #[error("invocation of relation {rel} failed")]
    RelationInvocation { rel: String },
    #[error("application of rule {relation}/{group}/{path} failed")]
    RuleApplication {
        relation: String,
        group: String,
        path: String,
    },
    #[error("application of table row {func}{args} failed")]
    TableRowApplication { func: String, args: String },
    #[error("invocation of function ${func}{targs} failed")]
    FunctionInvocation { func: String, targs: String },
    #[error("application of clause {func}{args} failed")]
    ClauseApplication { func: String, args: String },
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Error {
    pub kind: Box<ErrorKind>,
    pub span: Span,
    pub children: Vec<Error>,
}

impl Error {
    pub fn new(kind: ErrorKind, span: Span) -> Self {
        Self {
            kind: Box::new(kind),
            span,
            children: Vec::new(),
        }
    }

    pub fn execution(children: Vec<Error>) -> Self {
        Self {
            kind: Box::new(ErrorKind::Trace(TraceErrorKind::Execution)),
            span: Span::default(),
            children,
        }
    }

    pub fn at_if_missing(mut self, span: &Span) -> Self {
        if self.span == Span::default() && !matches!(*self.kind, ErrorKind::Guard(_)) {
            self.span = span.clone();
        }
        self
    }

    pub(super) fn undefined(kind: EntityKind, name: String, span: Span) -> Self {
        Self::new(
            ErrorKind::Context(ContextErrorKind::Undefined { kind, name }),
            span,
        )
    }

    pub(super) fn duplicate(kind: EntityKind, name: String, span: Span) -> Self {
        Self::new(
            ErrorKind::Context(ContextErrorKind::Duplicate { kind, name }),
            span,
        )
    }
}

impl From<ErrorKind> for Error {
    fn from(kind: ErrorKind) -> Self {
        Self::new(kind, Span::default())
    }
}

macro_rules! from_error {
    ($error:ty, $category:ident, $kind:ident, $variant:ident) => {
        impl From<$error> for Error {
            fn from(error: $error) -> Self {
                Self::new(
                    ErrorKind::$category($kind::$variant(error)),
                    Span::default(),
                )
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
        Self::new(
            ErrorKind::Runtime(RuntimeErrorKind::Type(error.kind)),
            error.span,
        )
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

struct MatchDisplay<'a>(&'a MatchError);
impl fmt::Display for MatchDisplay<'_> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.0 {
            MatchError::UndefinedType { name, .. } => write!(formatter, "undefined type {name}"),
            MatchError::UnexpectedTypeVariable { .. } => {
                formatter.write_str("unexpected type variable")
            }
            MatchError::TypeArgumentMismatch {
                expected, actual, ..
            } => write!(
                formatter,
                "expected {expected} type arguments, got {actual}"
            ),
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
        if matches!(*self.kind, ErrorKind::Trace(TraceErrorKind::Execution)) {
            fmt::Display::fmt(&TraceDisplay(&self.children), formatter)
        } else {
            fmt::Display::fmt(&TraceDisplay(std::slice::from_ref(self)), formatter)
        }
    }
}

struct TraceDisplay<'a>(&'a [Error]);
impl fmt::Display for TraceDisplay<'_> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        fn depth(trace: &Error) -> usize {
            1 + trace.children.iter().map(depth).max().unwrap_or(0)
        }
        fn traces(
            out: &mut impl fmt::Write,
            values: &[Error],
            indent: &str,
            run: usize,
            root: bool,
        ) -> fmt::Result {
            for (index, trace) in values.iter().enumerate() {
                if !root && depth(trace) > 10 {
                    traces(out, &trace.children, indent, run + 1, false)?;
                    continue;
                }
                if run > 0 {
                    writeln!(out, "{indent}│ ··· omitting {run} traces ···")?;
                }
                let last = index + 1 == values.len();
                let boundary = root || run > 0;
                write!(out, "{indent}")?;
                if !boundary {
                    write!(out, "{}", if last { "└── " } else { "├── " })?;
                }
                if trace.span != Span::default() {
                    writeln!(out, "{}", trace.span)?;
                    write!(out, "{indent}{}", if boundary { "" } else { "    " })?;
                }
                if values.len() > 1 {
                    write!(out, "{}. ", index + 1)?;
                }
                writeln!(out, "{}", trace.kind)?;
                let indent_sub = if boundary {
                    indent.to_owned()
                } else {
                    format!("{indent}{}", if last { "    " } else { "│   " })
                };
                traces(out, &trace.children, &indent_sub, 0, false)?;
            }
            Ok(())
        }
        traces(formatter, self.0, "", 0, true)
    }
}
