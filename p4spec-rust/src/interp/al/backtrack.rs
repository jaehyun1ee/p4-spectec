//! Recoverable mismatches and fatal failures during AL execution
//!
//! `choose_sequential` evaluates candidates until one succeeds or fails fatally.
//! Only mismatches try the next candidate. `nest` records call context without
//! changing that distinction; rendering the resulting trace tree is separate
//! from choosing a branch.

use crate::lang::common::source::Span;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FailTrace {
    pub span: Span,
    pub message: String,
    pub children: Vec<FailTrace>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Backtrack<T> {
    Ok(T),
    Err(Vec<FailTrace>),
    Unmatch(Vec<FailTrace>),
}

impl<T> Backtrack<T> {
    pub fn err(span: Span, message: impl Into<String>) -> Self {
        Self::Err(vec![FailTrace {
            span,
            message: message.into(),
            children: Vec::new(),
        }])
    }

    pub fn unmatch(span: Span, message: impl Into<String>) -> Self {
        Self::Unmatch(vec![FailTrace {
            span,
            message: message.into(),
            children: Vec::new(),
        }])
    }

    pub fn nest(self, span: Span, message: impl FnOnce() -> String) -> Self {
        match self {
            Self::Ok(value) => Self::Ok(value),
            Self::Err(children) => Self::Err(vec![FailTrace {
                span,
                message: message(),
                children,
            }]),
            Self::Unmatch(children) => Self::Unmatch(vec![FailTrace {
                span,
                message: message(),
                children,
            }]),
        }
    }

    pub fn and_then<U>(self, next: impl FnOnce(T) -> Backtrack<U>) -> Backtrack<U> {
        match self {
            Self::Ok(value) => next(value),
            Self::Err(traces) => Backtrack::Err(traces),
            Self::Unmatch(traces) => Backtrack::Unmatch(traces),
        }
    }
}

impl Backtrack<()> {
    pub fn check(condition: bool, span: Span, message: impl Into<String>) -> Self {
        if condition {
            Self::Ok(())
        } else {
            Self::err(span, message)
        }
    }
}

pub fn choose_sequential<C, T>(
    candidates: impl IntoIterator<Item = C>,
    mut evaluate: impl FnMut(&C) -> Backtrack<T>,
) -> Backtrack<T> {
    let mut traces = Vec::new();
    for candidate in candidates {
        match evaluate(&candidate) {
            Backtrack::Ok(value) => return Backtrack::Ok(value),
            Backtrack::Err(traces) => return Backtrack::Err(traces),
            Backtrack::Unmatch(mut traces_candidate) => traces.append(&mut traces_candidate),
        }
    }
    Backtrack::Unmatch(traces)
}
