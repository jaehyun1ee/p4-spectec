//! Structured diagnostics authored by elaboration
//!
//! Each constructor owns one stable `elab/...` code and preserves the source
//! spans available at its semantic check. Attempt frames remain uncoded context;
//! terminal causes keep their complete diagnostic payload in `Report`.

use crate::{
    diagnostic::{Diagnostic, Label, LabelStyle, Report, ReportKind, Severity},
    lang::common::source::Span,
};

mod argument;
mod declaration;
mod dimension;
mod exp;
mod premise;
mod typ;

pub(super) use argument::*;
pub(super) use declaration::*;
pub(super) use dimension::*;
pub(super) use exp::*;
pub(super) use premise::*;
pub(super) use typ::*;

/// Names a structured elaboration failure without adding a wrapper.
pub type ElabError = Box<Report>;

/// Labels the source occurrence responsible for a diagnostic.
fn primary(span: &Span) -> Label {
    Label { style: LabelStyle::Primary, span: span.clone(), message: String::new() }
}

/// Relates another source occurrence to the responsible occurrence.
fn related(span: &Span, message: impl Into<String>) -> Label {
    Label { style: LabelStyle::Secondary, span: span.clone(), message: message.into() }
}

/// Creates an elaboration diagnostic without reading source files.
fn diagnostic(
    severity: Severity,
    code: &str,
    message: impl Into<String>,
    labels: Vec<Label>,
    notes: Vec<String>,
) -> Diagnostic {
    Diagnostic {
        severity,
        code: Some(code.to_owned()),
        message: message.into(),
        labels,
        notes,
        source: "elab",
    }
}

/// Creates an error diagnostic for declaration constructors.
fn make_diagnostic(code: &str, message: String, labels: Vec<Label>) -> Diagnostic {
    diagnostic(Severity::Error, code, message, labels, Vec::new())
}

/// Creates a boxed error report.
fn cause(
    code: &str,
    message: impl Into<String>,
    labels: Vec<Label>,
    notes: Vec<String>,
) -> ElabError {
    Box::new(diagnostic(Severity::Error, code, message, labels, notes).into())
}

/// Creates a warning report.
fn warning(
    code: &str,
    message: impl Into<String>,
    labels: Vec<Label>,
    notes: Vec<String>,
) -> Report {
    diagnostic(Severity::Warning, code, message, labels, notes).into()
}

/// Creates an uncoded attempt-context frame.
pub(super) fn frame(span: &Span, message: impl Into<String>) -> Report {
    Report {
        kind: ReportKind::Frame { span: span.clone(), message: message.into() },
        children: Vec::new(),
    }
}
