//! Backtracking state for elaboration alternatives
//!
//! Many constructs have several readings, such as `a ++ b` being a list or a
//! text concatenation. `choose_sequential` runs the first alternative on a
//! copy of the context and falls back to the second, keeping the failure
//! traces of both so that `finish` can report the most informative error
//! when every alternative fails.

use crate::{lang::common::source::Span, runtime::ops::typ::TypeError};

use super::{ElabError, ElabErrorKind, context::Context, error::ElabTrace};

/// A successful elaboration result or recoverable backtracking failure.
pub(super) type Attempt<T> = Result<T, Backtrack>;

// == Attempt helpers

/// Fails an attempt with a single located error.
pub(super) fn fail<T>(error: ElabError) -> Attempt<T> {
    Err(error.into())
}

/// Fails an attempt without a trace, for alternatives that never apply.
pub(super) fn fail_silent<T>() -> Attempt<T> {
    Err(Backtrack::default())
}

/// Tries the first alternative and falls back to the second on failure.
///
/// Only the context of the successful alternative is kept; the traces of
/// both failures are merged when neither succeeds.
pub(super) fn choose_sequential<T>(
    ctx: &mut Context,
    first: impl FnOnce(&mut Context) -> Attempt<T>,
    second: impl FnOnce(&mut Context) -> Attempt<T>,
) -> Attempt<T> {
    // Run the first alternative on a copy of the context
    let mut ctx_first = ctx.clone();
    match first(&mut ctx_first) {
        // Commit the context of the successful alternative
        Ok(value) => {
            *ctx = ctx_first;
            Ok(value)
        }
        Err(failure) => {
            // Retry with the second alternative and merge both failures
            let mut ctx_second = ctx.clone();
            match second(&mut ctx_second) {
                Ok(value) => {
                    *ctx = ctx_second;
                    Ok(value)
                }
                Err(failure_second) => Err(failure.merge(failure_second)),
            }
        }
    }
}

/// Converts a finished attempt into a plain elaboration result.
pub(super) fn finish<T>(attempt: Attempt<T>) -> Result<T, ElabError> {
    attempt.map_err(Backtrack::into_error)
}

/// Backtracking state that accumulates elaboration traces for error reporting.
#[derive(Debug, Default)]
pub(super) struct Backtrack {
    traces: Vec<ElabTrace>,
}

impl Backtrack {
    /// Wraps the collected traces under a new parent error.
    pub(super) fn nest(self, error: ElabError) -> Self {
        Self { traces: vec![ElabTrace { error, children: self.traces }] }
    }

    /// Appends the traces of another failed alternative.
    pub(super) fn merge(mut self, mut other: Self) -> Self {
        self.traces.append(&mut other.traces);
        self
    }

    /// Visits a trace tree and keeps the most informative error.
    ///
    /// Located errors beat unlocated ones, specific kinds beat the generic
    /// no-match kind, and deeper errors beat shallower ones.
    fn best_error_in<'a>(
        trace: &'a ElabTrace,
        depth: usize,
        best: &mut Option<(usize, bool, bool, &'a ElabError)>,
    ) {
        let located = trace.error.span != Span::default();
        let specific = trace.error.kind != ElabErrorKind::NoMatchingAlternative;
        // Prefer located, then specific, then deeper errors
        if best.is_none_or(|(best_depth, best_located, best_specific, _)| {
            (located, specific, depth) > (best_located, best_specific, best_depth)
        }) {
            *best = Some((depth, located, specific, &trace.error));
        }
        for child in &trace.children {
            Self::best_error_in(child, depth + 1, best);
        }
    }

    /// Selects the best error among all traces and attaches the traces to it.
    pub(super) fn into_error(self) -> ElabError {
        let mut best = None;
        for trace in &self.traces {
            Self::best_error_in(trace, 0, &mut best);
        }
        // Fall back to a generic no-match error when no trace exists
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
        Self { traces: vec![ElabTrace::leaf(error)] }
    }
}

impl From<TypeError> for Backtrack {
    fn from(error: TypeError) -> Self {
        ElabError::from(error).into()
    }
}
