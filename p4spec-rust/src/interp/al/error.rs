//! Located AL lookup, host, and execution failures

use super::backtrack::FailTrace;
use crate::runner::{ExternError, InterfaceError};
use std::fmt;

use crate::lang::{common::source::Span, data::value::ValueError};

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
    #[error("{kind} `{name}` is undefined")]
    Undefined { kind: EntityKind, name: String },
    #[error("{kind} `{name}` was already defined")]
    Duplicate { kind: EntityKind, name: String },
    #[error("mismatch in optionality of iterated variables")]
    OptionalityMismatch,
    #[error("cannot transpose a matrix of value batches")]
    IterationLengthMismatch { expected: usize, actual: usize },
    #[error(transparent)]
    Value(#[from] ValueError),
    #[error(transparent)]
    Interface(#[from] InterfaceError),
    #[error(transparent)]
    Extern(#[from] ExternError),
    #[error("{}", TraceDisplay(.0))]
    Execution(Vec<FailTrace>),
}

#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
#[error("{kind} at {span}")]
pub struct Error {
    pub kind: ErrorKind,
    pub span: Span,
}

impl Error {
    pub fn new(kind: ErrorKind, span: Span) -> Self {
        Self { kind, span }
    }

    pub(super) fn undefined(kind: EntityKind, name: String, span: Span) -> Self {
        Self::new(ErrorKind::Undefined { kind, name }, span)
    }

    pub(super) fn duplicate(kind: EntityKind, name: String, span: Span) -> Self {
        Self::new(ErrorKind::Duplicate { kind, name }, span)
    }
}

impl From<InterfaceError> for Error {
    fn from(error: InterfaceError) -> Self {
        Self::new(ErrorKind::Interface(error), Span::default())
    }
}
impl From<ExternError> for Error {
    fn from(error: ExternError) -> Self {
        Self::new(ErrorKind::Extern(error), Span::default())
    }
}

struct TraceDisplay<'a>(&'a [FailTrace]);
impl fmt::Display for TraceDisplay<'_> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        fn depth(trace: &FailTrace) -> usize {
            1 + trace.children.iter().map(depth).max().unwrap_or(0)
        }
        fn traces(
            out: &mut impl fmt::Write,
            values: &[FailTrace],
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
                writeln!(out, "{}", trace.message)?;
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
