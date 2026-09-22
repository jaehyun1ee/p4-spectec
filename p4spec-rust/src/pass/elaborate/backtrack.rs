//! Three-state results for elaboration alternatives
//!
//! A mismatch permits another candidate to run on the original context.
//! A fatal failure stops the search immediately.
//! Both failure states retain complete reports in source candidate order.

use crate::{
    diagnostic::{LabelStyle, Report, ReportKind},
    lang::common::source::Span,
};

use super::{context::Context, error, error::ElabError};

// == Result

/// A successful elaboration, fatal failure, or recoverable mismatch.
#[derive(Debug)]
pub(super) enum Backtrack<T> {
    /// The operation produced a value.
    Success(T),
    /// The operation failed and no alternative may be tried.
    Fatal(Vec<Report>),
    /// The candidate did not apply and another candidate may be tried.
    Mismatch(Vec<Report>),
}

// == Macros

/// Builds [`Backtrack::Success`].
macro_rules! success {
    ($($value:tt)*) => {
        $crate::pass::elaborate::backtrack::Backtrack::Success($($value)*)
    };
}
pub(super) use success;

/// Builds [`Backtrack::Fatal`] from a report list.
macro_rules! fatal {
    (error: $error:expr $(,)?) => {
        $crate::pass::elaborate::backtrack::Backtrack::Fatal(vec![*$error])
    };
    ($($reports:tt)*) => {
        $crate::pass::elaborate::backtrack::Backtrack::Fatal($($reports)*)
    };
}
pub(super) use fatal;

/// Builds [`Backtrack::Mismatch`] from a report list.
macro_rules! mismatch {
    (error: $error:expr $(,)?) => {
        $crate::pass::elaborate::backtrack::Backtrack::Mismatch(vec![*$error])
    };
    (report: $report:expr $(,)?) => {
        $crate::pass::elaborate::backtrack::Backtrack::Mismatch(vec![$report])
    };
    ($($reports:tt)*) => {
        $crate::pass::elaborate::backtrack::Backtrack::Mismatch($($reports)*)
    };
}
pub(super) use mismatch;

/// Returns early while preserving fatal and mismatch states.
macro_rules! unwrap {
    ($result:expr) => {
        match $result {
            $crate::pass::elaborate::backtrack::success!(value) => value,
            $crate::pass::elaborate::backtrack::fatal!(reports) => {
                return $crate::pass::elaborate::backtrack::fatal!(reports)
            }
            $crate::pass::elaborate::backtrack::mismatch!(reports) => {
                return $crate::pass::elaborate::backtrack::mismatch!(reports)
            }
        }
    };
}
pub(super) use unwrap;

/// Promotes a plain elaboration result to fatal and returns its value.
macro_rules! unwrap_from_result {
    ($result:expr) => {
        match $result {
            Ok(value) => value,
            Err(error) => return $crate::pass::elaborate::backtrack::fatal!(vec![*error]),
        }
    };
}
pub(super) use unwrap_from_result;

// == Propagation and context

impl<T> Backtrack<T> {
    /// Maps a successful value while preserving either failure state.
    pub(super) fn map<U>(self, map: impl FnOnce(T) -> U) -> Backtrack<U> {
        match self {
            success!(value) => success!(map(value)),
            fatal!(reports) => fatal!(reports),
            mismatch!(reports) => mismatch!(reports),
        }
    }

    /// Promotes a mismatch to fatal at a non-backtracking boundary.
    pub(super) fn mismatch_as_failure(self) -> Self {
        match self {
            mismatch!(reports) => fatal!(reports),
            result => result,
        }
    }

    /// Wraps a recoverable mismatch under operation context.
    ///
    /// Fatal reports pass through unchanged so their direct cause is retained.
    pub(super) fn nest(self, span: Span, message: impl Into<String>) -> Self {
        match self {
            mismatch!(children) => mismatch!(vec![Report::frame(span, message, children)]),
            result => result,
        }
    }
}

// == Choice

/// Tries the second alternative only when the first mismatches.
///
/// Each candidate starts from the original context;
/// only the successful candidate is committed.
pub(super) fn choose_sequential<T>(
    ctx: &mut Context,
    first: impl FnOnce(&mut Context) -> Backtrack<T>,
    second: impl FnOnce(&mut Context) -> Backtrack<T>,
) -> Backtrack<T> {
    // Run the first alternative on a copy of the context
    let mut ctx_first = ctx.clone();
    match first(&mut ctx_first) {
        // Commit the context of the successful alternative
        success!(value) => {
            *ctx = ctx_first;
            success!(value)
        }
        // Stop without committing the failed candidate
        fatal!(reports) => fatal!(reports),
        mismatch!(reports) => {
            // Retry the second alternative from the original context
            let mut ctx_second = ctx.clone();
            match second(&mut ctx_second) {
                success!(value) => {
                    *ctx = ctx_second;
                    success!(value)
                }
                fatal!(reports) => fatal!(reports),
                mismatch!(mut reports_second) => {
                    let mut reports = reports;
                    reports.append(&mut reports_second);
                    mismatch!(reports)
                }
            }
        }
    }
}

// == Finishing

/// Converts a completed backtrack into a plain elaboration result.
pub(super) fn finish<T>(result: Backtrack<T>) -> Result<T, ElabError> {
    match result {
        success!(value) => Ok(value),
        fatal!(reports) | mismatch!(reports) => Err(finish_reports(reports)),
    }
}

/// Preserves one report unchanged and groups multiple alternative reports.
fn finish_reports(mut reports: Vec<Report>) -> ElabError {
    match reports.len() {
        0 => error::elaboration_alternative_missing(),
        1 => Box::new(reports.pop().expect("one report remains")),
        _ => {
            let span = reports.iter().find_map(report_span).unwrap_or_default();
            let report = Report::frame(span, "elaboration alternatives failed", reports);
            Box::new(report)
        }
    }
}

/// Finds the first located root span without inspecting report presentation.
fn report_span(report: &Report) -> Option<Span> {
    let span = match &report.kind {
        ReportKind::Frame { span, .. } => span,
        ReportKind::Cause(diagnostic) => diagnostic
            .labels
            .iter()
            .find(|label| label.style == LabelStyle::Primary)
            .map(|label| &label.span)?,
    };
    (*span != Span::default()).then(|| span.clone())
}
