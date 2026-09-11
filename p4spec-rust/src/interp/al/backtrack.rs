//! AL failure propagation and ordered candidate selection
//!
//! Mismatches try the next candidate; fatal failures stop evaluation. A
//! deterministic choice continues after one success and reports the first
//! two successful candidates. Error trees retain the reasons independently
//! of these control-flow outcomes.

use super::error::{Error, ErrorKind, GuardErrorKind};
use crate::lang::common::source::Span;
use std::convert::Infallible;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Backtrack<T, C = Infallible> {
    Ok(T),
    Err(Vec<Error>),
    Unmatch(Vec<Error>),
    Nondet(C, C),
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
            Backtrack::Nondet(never, _) => match never {},
            Backtrack::Err(mut traces) if is_guard(&traces) => Err(traces.remove(0)),
            Backtrack::Err(traces) | Backtrack::Unmatch(traces) => Err(Error::execution(traces)),
        }
    }
}

// = Propagation

macro_rules! backtrack {
    ($result:expr) => {
        match $result {
            $crate::interp::al::backtrack::Backtrack::Ok(value) => value,
            $crate::interp::al::backtrack::Backtrack::Err(traces) => {
                return $crate::interp::al::backtrack::Backtrack::Err(traces)
            }
            $crate::interp::al::backtrack::Backtrack::Nondet(first, second) => {
                return $crate::interp::al::backtrack::Backtrack::Nondet(first, second)
            }
            $crate::interp::al::backtrack::Backtrack::Unmatch(traces) => {
                return $crate::interp::al::backtrack::Backtrack::Unmatch(traces)
            }
        }
    };
}
pub(super) use backtrack;

macro_rules! backtrack_from_result {
    ($result:expr, $span:expr $(,)?) => {
        $crate::interp::al::backtrack::backtrack!(
            $crate::interp::al::backtrack::Backtrack::from_result($result, $span)
        )
    };
}
pub(super) use backtrack_from_result;

// Guard checks escape directly instead of acquiring backtracking traces
fn is_guard(errors: &[Error]) -> bool {
    matches!(errors, [error] if matches!(*error.kind, ErrorKind::Guard(_)))
}

impl<T> Backtrack<T> {
    pub(in crate::interp::al) fn guard(self) -> Self {
        match self {
            Self::Err(errors) => Self::Err(
                errors
                    .into_iter()
                    .map(|mut error| {
                        if !matches!(*error.kind, ErrorKind::Guard(_)) {
                            error.kind =
                                Box::new(ErrorKind::Guard(GuardErrorKind::Validation(error.kind)));
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

impl<T, C> Backtrack<T, C> {
    pub fn nest(self, span: Span, kind: impl FnOnce() -> ErrorKind) -> Self {
        match self {
            Self::Ok(value) => Self::Ok(value),
            Self::Err(children) if is_guard(&children) => Self::Err(children),
            Self::Err(children) => Self::Err(vec![Error {
                kind: Box::new(kind()),
                span,
                children,
            }]),
            Self::Unmatch(children) => Self::Unmatch(vec![Error {
                kind: Box::new(kind()),
                span,
                children,
            }]),
            Self::Nondet(first, second) => Self::Nondet(first, second),
        }
    }
}

// = Checks

impl Backtrack<()> {
    pub fn check(condition: bool, span: Span, kind: ErrorKind) -> Self {
        if condition {
            Self::Ok(())
        } else {
            Self::err(span, kind)
        }
    }
}

// = Choice

impl<T> Backtrack<T> {
    pub fn with_candidates<C>(self) -> Backtrack<T, C> {
        match self {
            Self::Ok(value) => Backtrack::Ok(value),
            Self::Err(errors) => Backtrack::Err(errors),
            Self::Unmatch(errors) => Backtrack::Unmatch(errors),
            Self::Nondet(never, _) => match never {},
        }
    }
}

pub fn choose_sequential<C, T>(
    candidates: impl IntoIterator<Item = C>,
    mut evaluate: impl FnMut(&C) -> Backtrack<T>,
) -> Backtrack<T> {
    let mut errors = Vec::new();
    for candidate in candidates {
        match evaluate(&candidate) {
            Backtrack::Ok(value) => return Backtrack::Ok(value),
            Backtrack::Err(errors) => return Backtrack::Err(errors),
            Backtrack::Unmatch(mut candidate_errors) => errors.append(&mut candidate_errors),
            Backtrack::Nondet(never, _) => match never {},
        }
    }
    Backtrack::Unmatch(errors)
}

pub fn choose_deterministic<C, T>(
    candidates: impl IntoIterator<Item = C>,
    mut evaluate: impl FnMut(&C) -> Backtrack<T>,
) -> Backtrack<T, C> {
    let mut success = None;
    let mut errors = Vec::new();
    for candidate in candidates {
        match evaluate(&candidate) {
            Backtrack::Ok(value) => {
                if let Some((first, _)) = success {
                    return Backtrack::Nondet(first, candidate);
                }
                success = Some((candidate, value));
                errors.clear();
            }
            Backtrack::Err(errors) => return Backtrack::Err(errors),
            Backtrack::Unmatch(mut candidate_errors) => {
                if success.is_none() {
                    errors.append(&mut candidate_errors);
                }
            }
            Backtrack::Nondet(never, _) => match never {},
        }
    }
    match success {
        Some((_, value)) => Backtrack::Ok(value),
        None => Backtrack::Unmatch(errors),
    }
}
