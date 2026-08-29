//! Backtracking state for elaboration alternatives

use crate::{lang::common::source::Span, runtime::types::TypeError};

use super::{ElabError, ElabErrorKind, error::ElabTrace};

/// A successful elaboration result or recoverable backtracking failure
pub(super) type Attempt<T> = Result<T, Backtrack>;

// == Attempt helpers

pub(super) fn fail<T>(error: ElabError) -> Attempt<T> {
    Err(error.into())
}

pub(super) fn fail_silent<T>() -> Attempt<T> {
    Err(Backtrack::default())
}

pub(super) fn choose_sequential<T>(
    first: impl FnOnce() -> Attempt<T>,
    second: impl FnOnce() -> Attempt<T>,
) -> Attempt<T> {
    match first() {
        Ok(value) => Ok(value),
        Err(failure) => match second() {
            Ok(value) => Ok(value),
            Err(failure_second) => Err(failure.merge(failure_second)),
        },
    }
}

pub(super) fn finish<T>(attempt: Attempt<T>) -> Result<T, ElabError> {
    attempt.map_err(Backtrack::into_error)
}

/// A backtracking state that accumulates elaboration traces for error reporting
#[derive(Debug, Default)]
pub(super) struct Backtrack {
    traces: Vec<ElabTrace>,
}

impl Backtrack {
    pub(super) fn nest(self, error: ElabError) -> Self {
        Self {
            traces: vec![ElabTrace {
                error,
                children: self.traces,
            }],
        }
    }

    pub(super) fn merge(mut self, mut other: Self) -> Self {
        self.traces.append(&mut other.traces);
        self
    }

    fn best_error_in<'a>(
        trace: &'a ElabTrace,
        depth: usize,
        best: &mut Option<(usize, bool, bool, &'a ElabError)>,
    ) {
        let located = trace.error.span != Span::default();
        let specific = trace.error.kind != ElabErrorKind::NoMatchingAlternative;
        if best.is_none_or(|(best_depth, best_located, best_specific, _)| {
            (located, specific, depth) > (best_located, best_specific, best_depth)
        }) {
            *best = Some((depth, located, specific, &trace.error));
        }
        for child in &trace.children {
            Self::best_error_in(child, depth + 1, best);
        }
    }

    pub(super) fn into_error(self) -> ElabError {
        let mut best = None;
        for trace in &self.traces {
            Self::best_error_in(trace, 0, &mut best);
        }
        best.map(|(_, _, _, error)| error.clone())
            .unwrap_or_else(|| {
                ElabError::new(
                    ElabErrorKind::NoMatchingAlternative,
                    Span::default(),
                    "no elaboration alternative matched",
                )
            })
            .with_traces(self.traces)
    }
}

impl From<ElabError> for Backtrack {
    fn from(error: ElabError) -> Self {
        Self {
            traces: vec![ElabTrace::leaf(error)],
        }
    }
}

impl From<TypeError> for Backtrack {
    fn from(error: TypeError) -> Self {
        ElabError::from(error).into()
    }
}
