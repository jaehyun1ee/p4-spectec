//! Backtracking state for elaboration alternatives

use crate::lang::common::source::Span;

use super::{ElabError, ElabErrorKind};

#[derive(Clone, Debug)]
pub(super) struct FailTrace {
    error: ElabError,
    children: Vec<FailTrace>,
}

impl FailTrace {
    fn leaf(error: ElabError) -> Self {
        Self {
            error,
            children: vec![],
        }
    }

    fn best_error<'a>(
        &'a self,
        depth: usize,
        best: &mut Option<(usize, bool, bool, &'a ElabError)>,
    ) {
        let located = self.error.span != Span::default();
        let specific = self.error.kind != ElabErrorKind::NoMatchingAlternative;
        if best.is_none_or(|(best_depth, best_located, best_specific, _)| {
            (located, specific, depth) > (best_located, best_specific, best_depth)
        }) {
            *best = Some((depth, located, specific, &self.error));
        }
        for child in &self.children {
            child.best_error(depth + 1, best);
        }
    }
}

/// Internal success or deferred failure state used while trying alternatives
#[derive(Clone, Debug)]
pub(super) enum Attempt<T> {
    Ok(T),
    Fail(Vec<FailTrace>),
}

impl<T> Attempt<T> {
    pub(super) fn ok(value: T) -> Self {
        Self::Ok(value)
    }

    pub(super) fn fail(error: ElabError) -> Self {
        Self::Fail(vec![FailTrace::leaf(error)])
    }

    pub(super) fn fail_silent() -> Self {
        Self::Fail(vec![])
    }

    pub(super) fn and_then<U>(self, f: impl FnOnce(T) -> Attempt<U>) -> Attempt<U> {
        match self {
            Self::Ok(value) => f(value),
            Self::Fail(traces) => Attempt::Fail(traces),
        }
    }

    pub(super) fn map<U>(self, f: impl FnOnce(T) -> U) -> Attempt<U> {
        self.and_then(|value| Attempt::Ok(f(value)))
    }

    pub(super) fn nest(self, error: ElabError) -> Self {
        match self {
            Self::Ok(value) => Self::Ok(value),
            Self::Fail(children) => Self::Fail(vec![FailTrace { error, children }]),
        }
    }

    pub(super) fn choose_sequential(
        first: impl FnOnce() -> Self,
        second: impl FnOnce() -> Self,
    ) -> Self {
        match first() {
            Self::Ok(value) => Self::Ok(value),
            Self::Fail(mut traces) => match second() {
                Self::Ok(value) => Self::Ok(value),
                Self::Fail(mut traces_second) => {
                    traces.append(&mut traces_second);
                    Self::Fail(traces)
                }
            },
        }
    }

    pub(super) fn commit(self) -> Result<T, ElabError> {
        match self {
            Self::Ok(value) => Ok(value),
            Self::Fail(traces) => {
                let mut best = None;
                for trace in &traces {
                    trace.best_error(0, &mut best);
                }
                match best {
                    Some((_, _, _, error)) => Err(error.clone()),
                    None => Err(ElabError::new(
                        ElabErrorKind::NoMatchingAlternative,
                        Span::default(),
                        "no elaboration alternative matched",
                    )),
                }
            }
        }
    }
}
