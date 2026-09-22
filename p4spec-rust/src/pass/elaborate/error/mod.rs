//! Elaboration report constructors and the private migration boundary
//!
//! Declaration checks construct reports directly through this facade.
//! The private migration error retains selection metadata and nested traces
//! for type, expression, and premise sites until those families are converted.
//! `into_report` transfers existing reports without rebuilding their payloads.

use crate::diagnostic::{Diagnostic, Label, LabelStyle, Report, ReportKind, Severity};
use thiserror::Error;

use crate::{
    lang::common::source::Span,
    runtime::ops::typ::{TypeError, TypeErrorKind},
};

mod declaration;
pub(super) use declaration::*;

/// Names a structured elaboration failure without adding a wrapper.
pub type ElabError = Box<Report>;

/// Creates a diagnostic authored by elaboration without reading source files.
fn make_diagnostic(code: &str, message: String, labels: Vec<Label>) -> Diagnostic {
    Diagnostic {
        severity: Severity::Error,
        code: Some(code.to_owned()),
        message,
        labels,
        notes: Vec::new(),
        source: "elab",
    }
}

/// Names the binding families that still use the private migration bridge.
#[derive(Clone, Copy, Debug, Error, PartialEq, Eq)]
pub(super) enum EntityKind {
    #[error("type")]
    Type,
    #[error("meta-variable")]
    MetaVariable,
}

/// Structural type expected by a failed elaboration alternative.
#[derive(Clone, Copy, Debug, Error, PartialEq, Eq)]
pub(super) enum TypeShape {
    #[error("text")]
    Text,
    #[error("iteration")]
    Iteration,
    #[error("tuple")]
    Tuple,
    #[error("list")]
    List,
    #[error("struct")]
    Struct,
}

/// Retains typed metadata for unmigrated failures and alternative selection.
#[derive(Clone, Debug, Error, PartialEq, Eq)]
pub(super) enum ElabErrorKind {
    #[error("duplicate {0}")]
    Duplicate(EntityKind),
    #[error("type operation failed: {0}")]
    Type(TypeErrorKind),
    #[error("cannot destruct type as {0}")]
    CannotDestructure(TypeShape),
    #[error("cannot infer expression type")]
    CannotInfer,
    #[error("operator is not defined for the operand types")]
    OperatorNotDefined,
    #[error("types do not match")]
    TypeMismatch,
    #[error("iteration dimensions do not match")]
    DimensionMismatch,
    #[error("invalid or empty iteration")]
    InvalidIteration,
    #[error("argument or parameter arity does not match")]
    ArityMismatch,
    #[error("identifier is invalid in this role")]
    InvalidIdentifier,
    #[error("variant cases are ambiguous")]
    AmbiguousVariant,
    #[error("type extension target is invalid")]
    InvalidTypeExtension,
    #[error("expression cannot be cast to the expected type")]
    InvalidCast,
    #[error("argument does not match its parameter")]
    InvalidArgument,
    #[error("premise is invalid")]
    InvalidPremise,
    #[error("rule is invalid")]
    InvalidRule,
    #[error("definition is invalid")]
    InvalidDefinition,
    #[error("input hint is invalid")]
    InvalidInputHint,
    #[error("no elaboration alternative matched")]
    NoMatchingAlternative,
}

/// One failed alternative together with the nested failures it collected.
#[derive(Debug)]
pub(super) struct ElabTrace {
    pub(super) error: MigrationError,
    pub(super) children: Vec<ElabTrace>,
}

impl ElabTrace {
    pub(super) fn leaf(error: MigrationError) -> Self {
        Self { error, children: vec![] }
    }
}

/// Bridges unmigrated type and premise failures to the public report boundary.
#[derive(Debug)]
pub(super) struct MigrationError {
    pub kind: ElabErrorKind,
    pub span: Span,
    diagnostic: String,
    traces: Vec<ElabTrace>,
    report: Option<ElabError>,
}

impl MigrationError {
    pub(crate) fn new(kind: ElabErrorKind, span: Span, diagnostic: impl Into<String>) -> Self {
        Self { kind, span, diagnostic: diagnostic.into(), traces: vec![], report: None }
    }

    /// Copies selection metadata while retaining complete causes in the trace tree.
    pub(super) fn selection(&self) -> Self {
        Self::new(self.kind.clone(), self.span.clone(), self.diagnostic.clone())
    }

    /// Converts remaining type, expression, and premise failures without flattening traces.
    pub(super) fn into_report(self) -> ElabError {
        // Already structured reports keep their complete payload and child order
        let mut report = if let Some(report) = self.report {
            report
        } else if self.traces.is_empty() {
            // Unconverted sites remain uncoded until their owning family migrates
            Box::new(
                Diagnostic {
                    severity: Severity::Error,
                    code: None,
                    message: self.diagnostic,
                    labels: vec![Label {
                        style: LabelStyle::Primary,
                        span: self.span,
                        message: String::new(),
                    }],
                    notes: Vec::new(),
                    source: "elab",
                }
                .into(),
            )
        } else {
            // A search summary groups the original causes without replacing them
            Box::new(Report {
                kind: ReportKind::Frame { span: self.span, message: self.diagnostic },
                children: Vec::new(),
            })
        };
        // Preserve both nested error traces and the ordered alternative children
        for trace in self.traces {
            report.children.push(trace.into_report());
        }
        report
    }

    pub(super) fn with_traces(mut self, traces: Vec<ElabTrace>) -> Self {
        self.traces = traces;
        self
    }

    /// Builds a duplicate-definition error for `name` at `span`.
    pub(crate) fn duplicate(entity: EntityKind, name: &str, span: Span) -> Self {
        Self::new(
            ElabErrorKind::Duplicate(entity),
            span,
            format!("{entity} `{name}` was already defined"),
        )
    }
}

impl From<TypeError> for MigrationError {
    fn from(error: TypeError) -> Self {
        let diagnostic = error.kind.to_string();
        Self::new(ElabErrorKind::Type(error.kind), error.span, diagnostic)
    }
}

impl From<ElabError> for MigrationError {
    fn from(report: ElabError) -> Self {
        // Selection metadata is local; the report itself remains untouched
        let (span, diagnostic) = match &report.kind {
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
        Self {
            kind: ElabErrorKind::InvalidDefinition,
            span,
            diagnostic,
            traces: Vec::new(),
            report: Some(report),
        }
    }
}

impl ElabTrace {
    /// Retains a trace's cause and all its nested alternatives in order.
    fn into_report(self) -> Report {
        let mut report = self.error.into_report();
        for trace in self.children {
            report.children.push(trace.into_report());
        }
        *report
    }
}
