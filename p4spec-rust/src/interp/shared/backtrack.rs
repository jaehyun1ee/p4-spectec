//! Recoverable mismatches and fatal errors from interpreter operations
//!
//! Stage-specific control stays in AL candidate selection and SL flow
//! evaluation; this result only propagates values and failures

use super::error::{Error, ErrorKind, GuardErrorKind};
use crate::lang::common::source::Span;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Backtrack<T> {
    Ok(T),
    Err(Vec<Error>),
    Unmatch(Vec<Error>),
}

// = Constructors

impl<T> Backtrack<T> {
    pub fn from_result(result: Result<T, impl Into<Error>>, span: &Span) -> Self {
        match result {
            Ok(value) => Self::Ok(value),
            Err(error) => Self::Err(vec![error.into().at_if_missing(span)]),
        }
    }

    pub fn err(span: Span, kind: ErrorKind) -> Self {
        Self::Err(vec![Error::new(kind, span)])
    }

    pub fn unmatch(span: Span, kind: ErrorKind) -> Self {
        Self::Unmatch(vec![Error::new(kind, span)])
    }
}

// = Finishing

impl<T> Backtrack<T> {
    pub fn finish(self) -> Result<T, Error> {
        match self {
            Backtrack::Ok(value) => Ok(value),
            Backtrack::Err(mut traces) if is_guard(&traces) => Err(traces.remove(0)),
            Backtrack::Err(traces) | Backtrack::Unmatch(traces) => Err(Error::execution(traces)),
        }
    }
}

// = Propagation

macro_rules! backtrack {
    ($result:expr) => {
        match $result {
            $crate::interp::shared::backtrack::Backtrack::Ok(value) => value,
            $crate::interp::shared::backtrack::Backtrack::Err(traces) => {
                return $crate::interp::shared::backtrack::Backtrack::Err(traces)
            }
            $crate::interp::shared::backtrack::Backtrack::Unmatch(traces) => {
                return $crate::interp::shared::backtrack::Backtrack::Unmatch(traces)
            }
        }
    };
}
pub(crate) use backtrack;

macro_rules! backtrack_from_result {
    ($result:expr, $span:expr $(,)?) => {
        $crate::interp::shared::backtrack::backtrack!(
            $crate::interp::shared::backtrack::Backtrack::from_result($result, $span)
        )
    };
}
pub(crate) use backtrack_from_result;

// Guard checks escape directly instead of acquiring backtracking traces
fn is_guard(errors: &[Error]) -> bool {
    matches!(errors, [error] if matches!(*error.kind, ErrorKind::Guard(_)))
}

impl<T> Backtrack<T> {
    pub(crate) fn guard(self) -> Self {
        match self {
            Self::Err(errors) => Self::Err(
                errors
                    .into_iter()
                    .map(|mut error| {
                        if !matches!(*error.kind, ErrorKind::Guard(_)) {
                            error.kind = Box::new(ErrorKind::Guard(GuardErrorKind::Validation(
                                error.kind.clone(),
                            )));
                        }
                        error
                    })
                    .collect(),
            ),
            result => result,
        }
    }
}

// = Nesting

impl<T> Backtrack<T> {
    pub fn nest(self, span: Span, kind: impl FnOnce() -> ErrorKind) -> Self {
        match self {
            Self::Ok(value) => Self::Ok(value),
            Self::Err(children) if is_guard(&children) => Self::Err(children),
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
    pub fn check(condition: bool, span: Span, kind: ErrorKind) -> Self {
        if condition { Self::Ok(()) } else { Self::err(span, kind) }
    }
}
