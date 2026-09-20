//! Recoverable mismatches and fatal errors from interpreter operations
//!
//! `Err` is fatal; `Unmatch` is a mismatch the caller may recover from
//! by trying another alternative.
//! Stage-specific control stays in AL candidate selection
//! and SL flow evaluation;
//! this result only propagates values and failures.

use super::error::{Error, ErrorKind};
use crate::lang::common::source::Span;

/// A value, a fatal error, or a recoverable mismatch.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Backtrack<T> {
    /// The operation produced a value.
    Ok(T),
    /// A fatal failure with its trace.
    Err(Vec<Error>),
    /// A mismatch the caller may recover from by trying another alternative.
    Unmatch(Vec<Error>),
}

// = Constructors

impl<T> Backtrack<T> {
    /// Lifts a plain result, locating an unlocated error at `span`.
    pub fn from_result(result: Result<T, impl Into<Error>>, span: &Span) -> Self {
        match result {
            Ok(value) => Self::Ok(value),
            Err(error) => Self::Err(vec![error.into().at_if_missing(span)]),
        }
    }

    /// A single fatal error at `span`.
    pub fn err(span: Span, kind: ErrorKind) -> Self {
        Self::Err(vec![Error::new(kind, span)])
    }

    /// A single mismatch at `span`.
    pub fn unmatch(span: Span, kind: ErrorKind) -> Self {
        Self::Unmatch(vec![Error::new(kind, span)])
    }
}

/// Builds `Backtrack::Ok`.
macro_rules! ok {
    ($($value:tt)*) => {
        $crate::interp::shared::backtrack::Backtrack::Ok($($value)*)
    };
}
pub(crate) use ok;

/// Builds `Backtrack::Err` from a span and kind, or from a trace list.
macro_rules! err {
    ($span:expr, $kind:expr $(,)?) => {
        $crate::interp::shared::backtrack::Backtrack::err($span, $kind)
    };
    ($($errors:tt)*) => {
        $crate::interp::shared::backtrack::Backtrack::Err($($errors)*)
    };
}
pub(crate) use err;

/// Builds `Backtrack::Unmatch` from a span and kind, or from a trace list.
macro_rules! unmatch {
    ($span:expr, $kind:expr $(,)?) => {
        $crate::interp::shared::backtrack::Backtrack::unmatch($span, $kind)
    };
    ($($errors:tt)*) => {
        $crate::interp::shared::backtrack::Backtrack::Unmatch($($errors)*)
    };
}
pub(crate) use unmatch;

// = Finishing

impl<T> Backtrack<T> {
    /// Finishes a backtrack; both failure kinds become execution errors.
    pub fn finish(self) -> Result<T, Error> {
        match self {
            ok!(value) => Ok(value),
            err!(traces) | unmatch!(traces) => Err(Error::execution(traces)),
        }
    }
}

// = Propagation

/// Returns early from the enclosing function on `Err` or `Unmatch`, like `?`.
macro_rules! unwrap {
    ($result:expr) => {
        match $result {
            $crate::interp::shared::backtrack::ok!(value) => value,
            $crate::interp::shared::backtrack::err!(traces) => {
                return $crate::interp::shared::backtrack::err!(traces)
            }
            $crate::interp::shared::backtrack::unmatch!(traces) => {
                return $crate::interp::shared::backtrack::unmatch!(traces)
            }
        }
    };
}
pub(crate) use unwrap;

/// Lifts a plain result at `span`, then unwraps it.
macro_rules! unwrap_from_result {
    ($result:expr, $span:expr $(,)?) => {
        $crate::interp::shared::backtrack::unwrap!(
            $crate::interp::shared::backtrack::Backtrack::from_result($result, $span)
        )
    };
}
pub(crate) use unwrap_from_result;

// = Nesting

impl<T> Backtrack<T> {
    /// Wraps the failure traces under a new parent error at `span`.
    pub fn nest(self, span: Span, kind: impl FnOnce() -> ErrorKind) -> Self {
        match self {
            Self::Ok(value) => Self::Ok(value),
            Self::Err(children) => {
                Self::Err(vec![Error { kind: Box::new(kind()), span, children }])
            }
            Self::Unmatch(children) => {
                Self::Unmatch(vec![Error { kind: Box::new(kind()), span, children }])
            }
        }
    }
}

// = Checks

impl Backtrack<()> {
    /// Fails with `kind` at `span` unless `condition` holds.
    pub fn check(condition: bool, span: Span, kind: ErrorKind) -> Self {
        if condition { Self::Ok(()) } else { Self::err(span, kind) }
    }
}
