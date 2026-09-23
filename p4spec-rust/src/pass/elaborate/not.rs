//! Structured evidence and representative reports for notation alternatives
//!
//! Notation matching carries comparison evidence beside its failure reports.
//! Variant checking selects a representative only after choosing the retained
//! recovery group, keeping every original report in candidate order.
//! Type failures and failures at different locations keep their report trees.

use std::cmp::Reverse;

use crate::{
    diagnostic::{Diagnostic, Report, ReportKind},
    lang::{common::source::Span, il::ast as il},
};

use super::{backtrack::Backtrack, error};

/// Carries notation evidence without changing elaboration recovery states.
pub(super) type NotationBacktrack<T> = Backtrack<T, NotationReport>;

/// Keeps a failure's reports and optional notation comparison together.
#[derive(Debug)]
pub(super) struct NotationReport {
    /// Retains causes independently of their presentation eligibility.
    pub reports: Vec<Report>,
    /// Describes only failures reached through notation matching.
    pub similarity: Option<NotationSimilarity>,
}

/// Records only token comparisons reached by notation matching.
#[derive(Clone, Debug)]
pub(super) struct NotationSimilarity {
    span: Span,
    shape_mismatch: bool,
    token_distance: usize,
    tokens_matched: usize,
}

/// Associates a failed candidate with its complete expected notation.
pub(super) struct NotationCaseReport<'a> {
    /// Preserves the candidate's complete notation and declaration span.
    pub not_typ_il: &'a il::NotTyp,
    /// Keeps the candidate's reports and comparison evidence together.
    pub report: NotationReport,
}

impl From<Vec<Report>> for NotationReport {
    fn from(reports: Vec<Report>) -> Self {
        Self { reports, similarity: None }
    }
}

impl NotationReport {
    /// Releases reports at a boundary that does not compare notations.
    pub(super) fn into_reports(self) -> Vec<Report> {
        self.reports
    }

    /// Records a mismatch between corresponding literal tokens.
    pub(super) fn token(report: Report, span: &Span, text_expect: &str, text: &str) -> Self {
        Self {
            reports: vec![report],
            similarity: Some(NotationSimilarity {
                span: span.clone(),
                shape_mismatch: false,
                token_distance: token_distance(text_expect, text),
                tokens_matched: 0,
            }),
        }
    }

    /// Records incompatible notation trees without guessing token positions.
    pub(super) fn shape(report: Report, span: &Span) -> Self {
        Self {
            reports: vec![report],
            similarity: Some(NotationSimilarity {
                span: span.clone(),
                shape_mismatch: true,
                token_distance: 0,
                tokens_matched: 0,
            }),
        }
    }

    /// Adds literal matches preceding the failure in the enclosing notation.
    pub(super) fn with_matched_tokens(mut self, tokens_matched: usize) -> Self {
        if let Some(similarity) = &mut self.similarity {
            similarity.tokens_matched += tokens_matched;
        }
        self
    }
}

/// Counts character edits between corresponding tokens.
fn token_distance(text_expect: &str, text: &str) -> usize {
    let chars: Vec<_> = text.chars().collect();
    let mut distances: Vec<_> = (0..=chars.len()).collect();
    // Extend one row per expected character without retaining the matrix
    for (idx_expect, char_expect) in text_expect.chars().enumerate() {
        let mut distance_diagonal = distances[0];
        distances[0] = idx_expect + 1;
        for (idx, char_actual) in chars.iter().enumerate() {
            let distance_previous = distances[idx + 1];
            distances[idx + 1] = (distance_diagonal + usize::from(char_expect != *char_actual))
                .min(distances[idx] + 1)
                .min(distance_previous + 1);
            distance_diagonal = distance_previous;
        }
    }
    distances[chars.len()]
}

/// Borrows a sole visible diagnostic without discarding a context frame.
fn diagnostic(report: &NotationReport) -> Option<&Diagnostic> {
    let [report] = report.reports.as_slice() else {
        return None;
    };
    match &report.kind {
        ReportKind::Cause(diagnostic) | ReportKind::Alternatives(diagnostic) => Some(diagnostic),
        ReportKind::Frame { .. } => None,
    }
}

/// Selects a representative only when all failures describe the same location.
fn representative<'a>(
    reports: &'a [NotationCaseReport<'_>],
) -> Option<(usize, &'a Diagnostic, &'a NotationSimilarity)> {
    let span = &reports.first()?.report.similarity.as_ref()?.span;
    let mut closest = None;
    // Validate every candidate before hiding any of its detailed diagnostics
    for (idx, report) in reports.iter().enumerate() {
        let similarity = report.report.similarity.as_ref()?;
        if similarity.span != *span {
            return None;
        }
        let diagnostic = diagnostic(&report.report)?;
        let key = (
            similarity.shape_mismatch,
            similarity.token_distance,
            Reverse(similarity.tokens_matched),
            idx,
        );
        // Keep declaration order for candidates with equal evidence
        if closest
            .as_ref()
            .is_none_or(|(key_previous, _, _)| key < *key_previous)
        {
            closest = Some((key, diagnostic, similarity));
        }
    }
    closest.map(|(key, diagnostic, similarity)| (key.3, diagnostic, similarity))
}

/// Summarizes retained variant failures without reordering their reports.
pub(super) fn summarize_variant(
    typ_expect_il: &il::Typ,
    span: &Span,
    mut reports: Vec<NotationCaseReport<'_>>,
) -> NotationReport {
    // A sole candidate already owns its complete failure context
    if reports.len() == 1 && reports[0].report.reports.len() == 1 {
        return reports.pop().expect("single notation candidate").report;
    }
    // Keep the selected child's scoped notes before adding this scope's list
    let summary = representative(&reports).map(|(idx, diagnostic, similarity)| {
        let mut diagnostic = diagnostic.clone();
        let not_typs_il: Vec<_> = reports
            .iter()
            .enumerate()
            .filter(|(idx_other, _)| *idx_other != idx)
            .map(|(_, report)| report.not_typ_il)
            .collect();
        diagnostic
            .notes
            .push(error::not::other_expected_notations(typ_expect_il, &not_typs_il));
        (diagnostic, similarity.clone())
    });
    let reports: Vec<_> = reports
        .into_iter()
        .flat_map(|report| report.report.reports)
        .collect();
    match summary {
        // Retain the full candidate tree beneath its representative
        Some((diagnostic, similarity)) => NotationReport {
            reports: vec![Report::alternatives(diagnostic, reports)],
            similarity: Some(similarity),
        },
        // Unsupported or differently located failures keep the original tree
        None => {
            let reports = if reports.len() == 1 {
                reports
            } else {
                vec![Report::frame(
                    span.clone(),
                    "expression does not match any variant case",
                    reports,
                )]
            };
            NotationReport::from(reports)
        }
    }
}
