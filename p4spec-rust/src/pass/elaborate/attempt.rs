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

    fn best_error<'a>(&'a self, depth: usize, best: &mut Option<(usize, bool, &'a ElabError)>) {
        let located = self.error.span != Span::default();
        if best.is_none_or(|(best_depth, best_located, _)| {
            (located && !best_located) || (located == best_located && depth > best_depth)
        }) {
            *best = Some((depth, located, &self.error));
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

    pub(super) fn choose_sequential<'a>(alternatives: Vec<Box<dyn FnOnce() -> Self + 'a>>) -> Self {
        let mut failures = vec![];
        for alternative in alternatives {
            match alternative() {
                Self::Ok(value) => return Self::Ok(value),
                Self::Fail(mut traces) => failures.append(&mut traces),
            }
        }
        Self::Fail(failures)
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
                    Some((_, _, error)) => Err(error.clone()),
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

#[cfg(test)]
mod tests {
    use std::cell::Cell;

    use crate::{
        lang::common::source::{Position, Span},
        pass::elaborate::{ElabError, ElabErrorKind, EntityKind},
    };

    use super::Attempt;

    fn span(label: &str) -> Span {
        Span::new(Position::new(label, 3, 1), Position::new(label, 3, 4))
    }

    fn error(kind: ElabErrorKind, label: &str) -> ElabError {
        ElabError::new(kind, span(label), label)
    }

    #[test]
    fn alternatives_stop_after_the_first_success() {
        let calls = Cell::new(0);
        let result = Attempt::choose_sequential(vec![
            Box::new(|| {
                calls.set(calls.get() + 1);
                Attempt::fail(error(ElabErrorKind::CannotInfer, "first"))
            }),
            Box::new(|| {
                calls.set(calls.get() + 1);
                Attempt::ok(7)
            }),
            Box::new(|| {
                calls.set(calls.get() + 1);
                Attempt::ok(9)
            }),
        ])
        .commit()
        .expect("successful alternative");

        assert_eq!(result, 7);
        assert_eq!(calls.get(), 2);
    }

    #[test]
    fn committing_alternatives_selects_the_deepest_located_cause() {
        let shallow_span = span("shallow");
        let deep_span = span("deep");
        let attempt = Attempt::<()>::choose_sequential(vec![
            Box::new(|| {
                Attempt::fail(ElabError::new(
                    ElabErrorKind::Undefined(EntityKind::Function),
                    shallow_span.clone(),
                    "undefined function",
                ))
            }),
            Box::new(|| {
                Attempt::fail(ElabError::new(
                    ElabErrorKind::TypeMismatch,
                    deep_span.clone(),
                    "type mismatch",
                ))
                .nest(error(ElabErrorKind::InvalidCast, "cast"))
                .nest(error(ElabErrorKind::NoMatchingAlternative, "expression"))
            }),
        ]);

        let error = attempt.commit().unwrap_err();

        assert_eq!(error.kind, ElabErrorKind::TypeMismatch);
        assert_eq!(error.span, deep_span);
    }

    #[test]
    fn composition_does_not_run_after_a_failure() {
        let calls = Cell::new(0);
        let failure_span = span("failure");
        let attempt = Attempt::<usize>::fail(ElabError::new(
            ElabErrorKind::InvalidArgument,
            failure_span.clone(),
            "invalid argument",
        ));

        let error = attempt
            .and_then(|_| {
                calls.set(calls.get() + 1);
                Attempt::ok(1)
            })
            .commit()
            .unwrap_err();

        assert_eq!(calls.get(), 0);
        assert_eq!(error.kind, ElabErrorKind::InvalidArgument);
        assert_eq!(error.span, failure_span);
    }

    #[test]
    fn located_context_wins_over_a_deeper_unlocated_cause() {
        let context_span = span("context");
        let attempt = Attempt::<()>::fail(ElabError::new(
            ElabErrorKind::TypeMismatch,
            Span::default(),
            "unlocated cause",
        ))
        .nest(ElabError::new(
            ElabErrorKind::InvalidArgument,
            context_span.clone(),
            "located context",
        ));

        let error = attempt.commit().unwrap_err();

        assert_eq!(error.kind, ElabErrorKind::InvalidArgument);
        assert_eq!(error.span, context_span);
    }
}
