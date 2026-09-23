//! Structured evidence and representative reports for notation alternatives
//!
//! Notation matching carries comparison evidence beside its failure reports.
//! Variant checking selects a representative only after choosing the retained
//! recovery group, keeping every original report in candidate order.
//! Type failures and failures at different locations keep their report trees.

use std::cmp::Reverse;

use crate::{
    diagnostic::{Diagnostic, Report},
    lang::{
        common::{
            notation::mixfix::{AtomPhrase, Mixfix},
            source::Span,
        },
        il::ast as il,
    },
};

use super::{
    backtrack::{Backtrack, mismatch, unavailable},
    error,
};

// == Failure payload

/// Carries notation evidence without changing elaboration recovery states.
pub(super) type NotBacktrack<T> = Backtrack<T, NotFailure>;

/// Keeps a failure's reports and optional notation comparison together.
#[derive(Debug)]
pub(super) struct NotFailure {
    /// Retains causes independently of their presentation eligibility.
    pub reports: Vec<Report>,
    /// Describes only failures reached through notation matching.
    pub similarity: Option<NotSimilarity>,
}

/// Records one comparison reached by notation matching.
#[derive(Clone, Debug)]
pub(super) struct NotSimilarity {
    /// Locates the compared expression or token.
    span: Span,
    /// Marks incompatible notation trees, which rank after token mismatches.
    is_shape_mismatch: bool,
    /// Counts character edits between corresponding tokens.
    token_distance: usize,
    /// Counts literals matched before the failure in enclosing notations.
    tokens_matched: usize,
}

/// Associates a failed candidate with its complete expected notation.
pub(super) struct NotCaseFailure<'a> {
    /// Preserves the candidate's complete notation and declaration span.
    pub not_typ_il: &'a il::NotTyp,
    /// Keeps the candidate's reports and comparison evidence together.
    pub failure: NotFailure,
}

impl From<Vec<Report>> for NotFailure {
    fn from(reports: Vec<Report>) -> Self {
        Self { reports, similarity: None }
    }
}

impl NotFailure {
    /// Records a mismatch between corresponding literal tokens.
    pub(super) fn token(report: Report, atom_expect: &AtomPhrase, atom: &AtomPhrase) -> Self {
        let text_expect = error::not::atom_text(atom_expect);
        let text = error::not::atom_text(atom);
        Self {
            reports: vec![report],
            similarity: Some(NotSimilarity {
                span: atom.span.clone(),
                is_shape_mismatch: false,
                token_distance: token_distance(&text_expect, &text),
                tokens_matched: 0,
            }),
        }
    }

    /// Records incompatible notation trees without guessing token positions.
    pub(super) fn shape(report: Report, span: &Span) -> Self {
        Self {
            reports: vec![report],
            similarity: Some(NotSimilarity {
                span: span.clone(),
                is_shape_mismatch: true,
                token_distance: 0,
                tokens_matched: 0,
            }),
        }
    }

    /// Adds literal matches preceding the failure in the enclosing notation.
    pub(super) fn add_matched_tokens(mut self, tokens_matched: usize) -> Self {
        if let Some(similarity) = &mut self.similarity {
            similarity.tokens_matched += tokens_matched;
        }
        self
    }
}

impl NotSimilarity {
    /// Orders closer evidence first.
    fn rank(&self) -> (bool, usize, Reverse<usize>) {
        (self.is_shape_mismatch, self.token_distance, Reverse(self.tokens_matched))
    }
}

// == Boundaries

// Plain results enter notation matching without a comparison
impl<T> Backtrack<T> {
    /// Lifts reports that carry no notation comparison.
    pub(super) fn without_similarity(self) -> NotBacktrack<T> {
        self.map_failure(NotFailure::from)
    }
}

impl<T> NotBacktrack<T> {
    /// Drops notation comparison at a boundary that does not rank candidates.
    pub(super) fn discard_similarity(self) -> Backtrack<T> {
        self.map_failure(|failure| failure.reports)
    }

    /// Wraps a recoverable failure under operation context.
    ///
    /// A frame groups failures that no longer describe one comparison.
    /// Fatal reports pass through unchanged so their direct cause is retained.
    pub(super) fn nest(self, span: Span, message: impl Into<String>) -> Self {
        match self {
            unavailable!(failure) => {
                unavailable!(report: Report::frame(span, message, failure.reports))
            }
            mismatch!(failure) => mismatch!(report: Report::frame(span, message, failure.reports)),
            result => result,
        }
    }
}

// == Variant summaries

/// Summarizes retained variant failures without reordering their reports.
pub(super) fn summarize_variant(
    typ_expect_il: &il::Typ,
    mut failures: Vec<NotCaseFailure<'_>>,
) -> NotFailure {
    // A sole candidate already owns its complete failure context
    if matches!(failures.as_slice(), [case] if case.failure.reports.len() == 1) {
        return failures.pop().expect("sole notation candidate").failure;
    }
    // Keep the selected child's scoped notes before adding this scope's list
    let summary = representative(&failures).map(|(idx, diagnostic)| {
        let mut diagnostic = diagnostic.clone();
        let not_typs_il: Vec<_> = failures
            .iter()
            .enumerate()
            .filter(|(idx_other, _)| *idx_other != idx)
            .map(|(_, case)| case.not_typ_il)
            .collect();
        diagnostic
            .notes
            .push(error::not::other_expected_notations(typ_expect_il, &not_typs_il));
        (diagnostic, failures[idx].failure.similarity.clone())
    });
    let reports: Vec<_> = failures
        .into_iter()
        .flat_map(|case| case.failure.reports)
        .collect();
    match summary {
        // Retain the full candidate tree beneath its representative
        Some((diagnostic, similarity)) => {
            NotFailure { reports: vec![Report::representative(diagnostic, reports)], similarity }
        }
        // Unsupported or differently located failures keep the original tree
        None => NotFailure::from(reports),
    }
}

/// Selects the closest candidate only when every failure describes one location.
fn representative<'a>(cases: &'a [NotCaseFailure<'_>]) -> Option<(usize, &'a Diagnostic)> {
    let span = &cases.first()?.failure.similarity.as_ref()?.span;
    // Hide detailed diagnostics only when every candidate is comparable
    let candidates: Vec<_> = cases
        .iter()
        .map(|case| {
            let similarity = case.failure.similarity.as_ref()?;
            let [report] = case.failure.reports.as_slice() else {
                return None;
            };
            (similarity.span == *span).then_some((similarity.rank(), report.diagnostic()?))
        })
        .collect::<Option<_>>()?;
    // `min_by_key` keeps declaration order among equal evidence
    candidates
        .into_iter()
        .enumerate()
        .min_by_key(|(_, (rank, _))| *rank)
        .map(|(idx, (_, diagnostic))| (idx, diagnostic))
}

// == Token comparison

/// Counts literals in notation that has already elaborated successfully.
pub(super) fn count_atoms(not_exp_il: &il::NotExp) -> usize {
    match not_exp_il {
        Mixfix::Arg(_) => 0,
        Mixfix::Atom(_) => 1,
        Mixfix::Seq(not_exps_il) => not_exps_il.iter().map(count_atoms).sum(),
        Mixfix::Infix(not_exp_l_il, _, not_exp_r_il) => {
            1 + count_atoms(not_exp_l_il) + count_atoms(not_exp_r_il)
        }
        Mixfix::Brack(_, not_exp_il, _) => 2 + count_atoms(not_exp_il),
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
        for (idx, char) in chars.iter().enumerate() {
            let distance_previous = distances[idx + 1];
            distances[idx + 1] = (distance_diagonal + usize::from(char_expect != *char))
                .min(distances[idx] + 1)
                .min(distance_previous + 1);
            distance_diagonal = distance_previous;
        }
    }
    distances[chars.len()]
}
