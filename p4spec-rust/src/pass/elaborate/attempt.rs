//! Backtracking state for elaboration alternatives
//!
//! Many constructs have several readings,
//! such as `a ++ b` being a list or a text concatenation.
//! `choose_sequential` isolates each branch's context and commits only a winner.
//! Failed branches retain complete reports while flat local metadata selects
//! the most informative summary without inspecting diagnostic presentation.

use crate::{
    diagnostic::{LabelStyle, Report, ReportKind},
    lang::common::source::Span,
};

use super::{context::Context, error, error::ElabError};

/// A successful elaboration result or recoverable backtracking failure.
pub(super) type Attempt<T> = Result<T, Backtrack>;

/// Distinguishes broad no-match context from a concrete failed check.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub(super) enum Specificity {
    Generic,
    Specific,
}

/// Summary used only to select the final attempt frame.
#[derive(Debug)]
struct Selection {
    span: Span,
    message: String,
    located: bool,
    specificity: Specificity,
    depth: usize,
}

impl Selection {
    /// Builds local selection metadata without consulting diagnostic presentation.
    fn new(span: Span, message: String, specificity: Specificity) -> Self {
        let located = span != Span::default();
        Self { span, message, located, specificity, depth: 0 }
    }

    /// Orders candidates by location, specificity, then nesting depth.
    fn outranks(&self, other: &Self) -> bool {
        (self.located, self.specificity, self.depth)
            > (other.located, other.specificity, other.depth)
    }
}

// == Attempt helpers

/// Fails an attempt with a concrete structured cause.
pub(super) fn fail<T>(error: ElabError) -> Attempt<T> {
    Err(error.into())
}

/// Fails an attempt with broad no-match context.
pub(super) fn fail_generic<T>(report: Report) -> Attempt<T> {
    Err(Backtrack::from_report(report, Specificity::Generic))
}

/// Fails an attempt without a report, for alternatives that never apply.
pub(super) fn fail_silent<T>() -> Attempt<T> {
    Err(Backtrack::default())
}

/// Tries the first alternative and falls back to the second on failure.
///
/// Only the context of the successful alternative is kept;
/// the reports of both failures are merged when neither succeeds.
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

/// Backtracking state with a report forest and flat selection metadata.
#[derive(Debug, Default)]
pub(super) struct Backtrack {
    reports: Vec<Report>,
    selection: Option<Selection>,
}

impl Backtrack {
    /// Creates a failure while preserving the incoming report unchanged.
    pub(super) fn from_report(report: Report, specificity: Specificity) -> Self {
        let (span, message) = match &report.kind {
            ReportKind::Frame { span, message } => (span.clone(), message.clone()),
            ReportKind::Cause(diagnostic) => {
                let span = diagnostic
                    .labels
                    .iter()
                    .find(|label| label.style == LabelStyle::Primary)
                    .map(|label| label.span.clone())
                    .unwrap_or_default();
                (span, diagnostic.message.clone())
            }
        };
        Self { reports: vec![report], selection: Some(Selection::new(span, message, specificity)) }
    }

    /// Wraps the collected reports under a new attempt-context frame.
    pub(super) fn nest(mut self, span: Span, message: impl Into<String>) -> Self {
        if let Some(selection) = &mut self.selection {
            selection.depth += 1;
        }
        let message = message.into();
        let selection_parent = Selection::new(span.clone(), message.clone(), Specificity::Generic);
        if self
            .selection
            .as_ref()
            .is_none_or(|selection| selection_parent.outranks(selection))
        {
            self.selection = Some(selection_parent);
        }
        self.reports =
            vec![Report { kind: ReportKind::Frame { span, message }, children: self.reports }];
        self
    }

    /// Appends the reports of another failed alternative.
    pub(super) fn merge(mut self, mut other: Self) -> Self {
        if let Some(selection_other) = other.selection.take()
            && self
                .selection
                .as_ref()
                .is_none_or(|selection| selection_other.outranks(selection))
        {
            self.selection = Some(selection_other);
        }
        self.reports.append(&mut other.reports);
        self
    }

    /// Selects the best summary and attaches every full report beneath it.
    pub(super) fn into_error(self) -> ElabError {
        match self.selection {
            Some(selection) => {
                let mut report = error::frame(&selection.span, selection.message);
                report.children = self.reports;
                Box::new(report)
            }
            None => error::elaboration_alternative_missing(),
        }
    }
}

impl From<ElabError> for Backtrack {
    fn from(error: ElabError) -> Self {
        Self::from_report(*error, Specificity::Specific)
    }
}
