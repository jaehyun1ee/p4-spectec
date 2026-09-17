//! Recoverable mismatches and fatal errors from interpreter operations
//!
//! Stage-specific control stays in AL candidate selection and SL flow
//! evaluation; this result only propagates values and failures

use super::error::{Error, ErrorKind};
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

macro_rules! ok {
    ($($value:tt)*) => {
        $crate::interp::shared::backtrack::Backtrack::Ok($($value)*)
    };
}
pub(crate) use ok;

macro_rules! err {
    ($span:expr, $kind:expr $(,)?) => {
        $crate::interp::shared::backtrack::Backtrack::err($span, $kind)
    };
    ($($errors:tt)*) => {
        $crate::interp::shared::backtrack::Backtrack::Err($($errors)*)
    };
}
pub(crate) use err;

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
    pub fn finish(self) -> Result<T, Error> {
        match self {
            ok!(value) => Ok(value),
            err!(traces) | unmatch!(traces) => Err(Error::execution(traces)),
        }
    }
}

// = Propagation

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
    pub fn check(condition: bool, span: Span, kind: ErrorKind) -> Self {
        if condition { Self::Ok(()) } else { Self::err(span, kind) }
    }
}
