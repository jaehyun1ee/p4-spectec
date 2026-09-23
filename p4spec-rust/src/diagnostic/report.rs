//! Source-independent diagnostics and recursive report trees
//!
//! Reports retain context frames, causes, and representatives of alternatives.
//! All node kinds keep nested reports in the same ordered `children` field.
//! Diagnostics retain codes, labels, and notes without owning descendants.
//! Report destruction drains descendants iteratively to bound stack use.

use std::fmt;

use crate::lang::common::source::Span;

use super::{LabelStyle, Severity};

// = Labels

/// Associates a source span with its role and explanation.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Label {
    /// Distinguishes responsible locations from related locations.
    pub style: LabelStyle,
    /// Preserves the original source coordinates.
    pub span: Span,
    /// Explains why the span is relevant.
    pub message: String,
}

impl Label {
    /// Labels the source occurrence responsible for a diagnostic.
    pub fn primary(span: &Span, message: impl Into<String>) -> Self {
        Self { style: LabelStyle::Primary, span: span.clone(), message: message.into() }
    }

    /// Relates another source occurrence to the responsible occurrence.
    pub fn secondary(span: &Span, message: impl Into<String>) -> Self {
        Self { style: LabelStyle::Secondary, span: span.clone(), message: message.into() }
    }
}

// = Diagnostic data

/// Carries one diagnostic without descendants, source text, or terminal policy.
#[derive(Clone, Debug)]
pub struct Diagnostic {
    /// Describes presentation severity, not recovery behavior.
    pub severity: Severity,
    /// Identifies the check using its producer and subject.
    pub code: Option<String>,
    /// Summarizes the failure independently of its labels.
    pub message: String,
    /// Identifies responsible and related source spans.
    pub labels: Vec<Label>,
    /// Supplies supplementary explanations.
    pub notes: Vec<String>,
    /// Identifies the component that authored the diagnostic.
    pub source: &'static str,
}

impl Diagnostic {
    /// Constructs diagnostic data without source loading or rendering policy.
    pub fn new(
        source: &'static str,
        severity: Severity,
        code: Option<String>,
        message: impl Into<String>,
        labels: Vec<Label>,
        notes: Vec<String>,
    ) -> Self {
        Self { severity, code, message: message.into(), labels, notes, source }
    }
}

impl fmt::Display for Diagnostic {
    fn fmt(&self, fmt: &mut fmt::Formatter<'_>) -> fmt::Result {
        // Translate severity without traversing sources or causes
        let severity = match self.severity {
            Severity::Bug => "bug",
            Severity::Error => "error",
            Severity::Warning => "warning",
            Severity::Note => "note",
            Severity::Help => "help",
        };
        write!(fmt, "{severity}")?;
        // Keep the stable identifier visible in fallback summaries
        if let Some(code) = &self.code {
            write!(fmt, "[{code}]")?;
        }
        // Leave notes and labels for the renderer
        write!(fmt, ": {}", self.message)
    }
}

// = Report trees

/// Preserves one diagnostic or context node and its ordered child reports.
pub struct Report {
    /// Distinguishes operation context from structured diagnostic content.
    pub kind: ReportKind,
    /// Retains nested causes and alternative order for every node kind.
    pub children: Vec<Report>,
}

/// Describes a report node independently of its children.
#[derive(Debug)]
pub enum ReportKind {
    /// Groups child reports under a located evaluation or search context.
    Frame {
        /// Locates the operation being attempted.
        span: Span,
        /// Describes the operation being attempted.
        message: String,
    },
    /// Retains a cause with its own code, severity, labels, and notes.
    Cause(Diagnostic),
    /// Displays one representative diagnostic above its retained alternatives.
    Representative(Diagnostic),
}

impl Report {
    /// Groups ordered child reports under an uncoded source context.
    pub fn frame(span: Span, message: impl Into<String>, children: Vec<Report>) -> Self {
        Self { kind: ReportKind::Frame { span, message: message.into() }, children }
    }

    /// Retains ordered alternative failures beneath their representative.
    pub fn representative(diagnostic: Diagnostic, children: Vec<Report>) -> Self {
        Self { kind: ReportKind::Representative(diagnostic), children }
    }

    /// Borrows the diagnostic of a cause or representative node.
    pub fn diagnostic(&self) -> Option<&Diagnostic> {
        match &self.kind {
            ReportKind::Cause(diagnostic) | ReportKind::Representative(diagnostic) => {
                Some(diagnostic)
            }
            ReportKind::Frame { .. } => None,
        }
    }

    /// Mutably borrows the diagnostic of a cause or representative node.
    pub fn diagnostic_mut(&mut self) -> Option<&mut Diagnostic> {
        match &mut self.kind {
            ReportKind::Cause(diagnostic) | ReportKind::Representative(diagnostic) => {
                Some(diagnostic)
            }
            ReportKind::Frame { .. } => None,
        }
    }
}

impl From<Diagnostic> for Report {
    fn from(diagnostic: Diagnostic) -> Self {
        Self { kind: ReportKind::Cause(diagnostic), children: Vec::new() }
    }
}

impl fmt::Display for Report {
    fn fmt(&self, fmt: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self.kind {
            ReportKind::Frame { message, .. } => write!(fmt, "note: {message}"),
            ReportKind::Cause(diagnostic) | ReportKind::Representative(diagnostic) => {
                fmt::Display::fmt(diagnostic, fmt)
            }
        }
    }
}

impl std::error::Error for Report {}

impl Drop for Report {
    fn drop(&mut self) {
        // Detach descendants before dropping each node
        let mut pending = std::mem::take(&mut self.children);
        while let Some(mut report) = pending.pop() {
            pending.append(&mut report.children);
        }
    }
}

impl fmt::Debug for Report {
    fn fmt(&self, fmt: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt.debug_struct("Report")
            .field("kind", &self.kind)
            .field("children", &self.children.len())
            .finish()
    }
}
