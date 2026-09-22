//! Source-independent diagnostic records and causal traces
//!
//! Labels identify responsible and related spans.
//! Trace frames add evaluation context;
//! diagnostic children retain their own codes, labels, and notes.
//! Trace destruction drains descendants iteratively to bound stack use.

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

// = Reports

/// Carries one diagnostic without source text or terminal policy.
#[derive(Debug)]
pub struct Report {
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
    /// Preserves causal context and alternative order.
    pub traces: Vec<Trace>,
}

impl fmt::Display for Report {
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
        // Leave notes and traces for the renderer
        write!(fmt, ": {}", self.message)
    }
}

impl std::error::Error for Report {}

// = Traces

/// Preserves a context frame or an independently structured cause.
pub enum Trace {
    /// Groups causes under a located evaluation or search context.
    Frame {
        /// Locates the operation being attempted.
        span: Span,
        /// Describes the operation being attempted.
        message: String,
        /// Retains nested causes in their original order.
        children: Vec<Trace>,
    },
    /// Retains a cause with its own code, severity, labels, and notes.
    Diagnostic(Box<Report>),
}

impl Trace {
    fn children_mut(&mut self) -> &mut Vec<Self> {
        match self {
            Self::Frame { children, .. } => children,
            Self::Diagnostic(report) => &mut report.traces,
        }
    }
}

impl Drop for Trace {
    fn drop(&mut self) {
        // Detach descendants before dropping each node
        let mut pending = std::mem::take(self.children_mut());
        while let Some(mut trace) = pending.pop() {
            pending.append(trace.children_mut());
        }
    }
}

impl fmt::Debug for Trace {
    fn fmt(&self, fmt: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Frame { span, message, children } => fmt
                .debug_struct("Frame")
                .field("span", span)
                .field("message", message)
                .field("children", &children.len())
                .finish(),
            Self::Diagnostic(report) => fmt
                .debug_struct("Diagnostic")
                .field("summary", &format_args!("{report}"))
                .field("children", &report.traces.len())
                .finish(),
        }
    }
}
