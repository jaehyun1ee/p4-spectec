//! Structured diagnostics authored by algorithmic conversion
//!
//! Constructors retain the spans at binding and table checks.
//! Recoverable anti-unification mismatches stay local to overlap analysis;
//! only terminal failures become reports at their owning boundary.

use crate::{
    diagnostic::{Diagnostic, Label, Report, Severity},
    lang::{common::source::Span, hints::input::InputError},
    runtime::ops::typ::TypeError,
};

mod binding;
mod otherwise;
mod table;

pub(super) use binding::*;
pub(super) use otherwise::*;
pub(super) use table::*;

/// Names an algorithmic conversion report without adding a wrapper.
pub type AlgoError = Box<Report>;

/// Creates an algorithmic error without reading source files.
fn cause(
    code: &str,
    message: impl Into<String>,
    labels: Vec<Label>,
    notes: Vec<String>,
) -> AlgoError {
    Box::new(
        Diagnostic::new("algo", Severity::Error, Some(code.to_owned()), message, labels, notes)
            .into(),
    )
}

const TYPE_OPERATION_INVALID: &str = "algo/type-operation-invalid";

/// Promotes a terminal type operation failure at its original location.
pub(crate) fn type_operation_invalid(error: TypeError) -> AlgoError {
    cause(
        TYPE_OPERATION_INVALID,
        format!("type operation failed: {}", error.kind),
        vec![Label::primary(&error.span, "")],
        vec![],
    )
}

const TYPE_UNDEFINED: &str = "algo/type-undefined";

/// Reports a missing type in directly supplied IL.
pub(crate) fn type_undefined(id: &crate::lang::common::Id) -> AlgoError {
    cause(
        TYPE_UNDEFINED,
        format!("type `{}` is not defined", id.node),
        vec![Label::primary(&id.span, "")],
        vec![],
    )
}

const TYPE_ARGUMENT_ARITY_MISMATCH: &str = "algo/type-argument-arity-mismatch";

/// Reports invalid type arguments in directly supplied IL.
pub(crate) fn type_argument_arity_mismatch(
    span: &Span,
    expected: usize,
    actual: usize,
) -> AlgoError {
    cause(
        TYPE_ARGUMENT_ARITY_MISMATCH,
        format!("type argument arity mismatch: expected {expected}, got {actual}"),
        vec![Label::primary(span, "")],
        vec![],
    )
}

const RELATION_INPUT_HINT_INVALID: &str = "algo/relation-input-hint-invalid";

/// Reports an invalid relation input hint in directly supplied IL.
pub(crate) fn relation_input_hint_invalid(error: InputError, span: Span) -> AlgoError {
    cause(
        RELATION_INPUT_HINT_INVALID,
        format!("invalid relation input hint: {error}"),
        vec![Label::primary(&span, "")],
        vec![],
    )
}

const RULE_INPUT_MISMATCH: &str = "algo/rule-input-mismatch";

/// Reports input templates that cannot overlap in directly supplied IL.
pub(crate) fn rule_input_mismatch(span: &Span) -> AlgoError {
    cause(
        RULE_INPUT_MISMATCH,
        "cannot anti-unify rule inputs",
        vec![Label::primary(span, "")],
        vec![],
    )
}
