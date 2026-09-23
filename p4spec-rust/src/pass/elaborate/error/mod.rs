//! Structured diagnostics authored by elaboration
//!
//! Each constructor owns one stable `elab/...` code and preserves the source
//! spans available at its semantic check. Attempt frames remain uncoded context;
//! terminal causes keep their complete diagnostic payload in [`Report`].

use crate::diagnostic::{Diagnostic, Label, Report, Severity};

mod arg;
mod decl;
mod dim;
mod exp;
mod expect;
mod not;
mod prem;
mod typ;

pub(super) use arg::*;
pub(super) use decl::*;
pub(super) use dim::*;
pub(super) use exp::*;
pub(super) use expect::*;
pub(super) use not::*;
pub(super) use prem::*;
pub(super) use typ::*;

/// Names a structured elaboration failure without adding a wrapper.
pub type ElabError = Box<Report>;

/// Creates an elaboration diagnostic without reading source files.
fn diagnostic(
    severity: Severity,
    code: &str,
    message: impl Into<String>,
    labels: Vec<Label>,
    notes: Vec<String>,
) -> Diagnostic {
    Diagnostic::new("elab", severity, Some(code.to_owned()), message, labels, notes)
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
