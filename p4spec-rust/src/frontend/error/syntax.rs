//! Diagnostics for grammar constraints
//!
//! Constructors retain the parser's responsible locations
//! for relation signatures, type definitions, and syntax declarations.

use crate::diagnostic::{Label, LabelStyle};
use crate::lang::common::source::Span;

use super::{FrontendError, make_diagnostic};

// = Relation signatures

const RELATION_SIGNATURE_INVALID: &str = "parse/relation-signature-invalid";

/// Reports a plain type used as a relation signature.
pub(crate) fn relation_signature_invalid(span: Span) -> FrontendError {
    let mut diagnostic = make_diagnostic(
        RELATION_SIGNATURE_INVALID,
        "relation signature must be a notation type".to_owned(),
        vec![Label {
            style: LabelStyle::Primary,
            span,
            message: "expected a notation type".to_owned(),
        }],
    );
    diagnostic.notes.push("A notation type includes literal tokens like `|-` or `:` that rules pattern-match against. A bare type like `nat` names a set of values without any tokens, so it cannot serve as a relation signature.".to_owned());
    Box::new(diagnostic.into())
}

// = Type definitions

const STRUCT_FIELD_MISSING: &str = "parse/struct-field-missing";

/// Reports a struct type without fields.
pub(crate) fn struct_field_missing(span: Span) -> FrontendError {
    let diagnostic = make_diagnostic(
        STRUCT_FIELD_MISSING,
        "empty struct type".to_owned(),
        vec![Label {
            style: LabelStyle::Primary,
            span,
            message: "expected at least one field".to_owned(),
        }],
    );
    Box::new(diagnostic.into())
}

const VARIANT_CASE_MISSING: &str = "parse/variant-case-missing";

/// Reports a variant type without cases.
pub(crate) fn variant_case_missing(span: Span) -> FrontendError {
    let diagnostic = make_diagnostic(
        VARIANT_CASE_MISSING,
        "empty variant type".to_owned(),
        vec![Label {
            style: LabelStyle::Primary,
            span,
            message: "expected at least one case".to_owned(),
        }],
    );
    Box::new(diagnostic.into())
}

const PLAIN_TYPE_HINT_UNSUPPORTED: &str = "parse/plain-type-hint-unsupported";

/// Reports hints attached to a plain type definition.
pub(crate) fn plain_type_hint_unsupported(span: Span) -> FrontendError {
    let mut diagnostic = make_diagnostic(
        PLAIN_TYPE_HINT_UNSUPPORTED,
        "hints are not allowed on a plain type definition".to_owned(),
        vec![Label {
            style: LabelStyle::Primary,
            span,
            message: "plain types inherit their hints".to_owned(),
        }],
    );
    diagnostic.notes.push("A plain type definition aliases an existing type, as in `syntax x = nat`. It inherits the aliased type's hints and cannot declare its own.".to_owned());
    Box::new(diagnostic.into())
}

// = Syntax declarations

const SYNTAX_BODY_MISSING: &str = "parse/syntax-body-missing";

/// Reports a syntax definition without a body.
pub(crate) fn syntax_body_missing(span: Span) -> FrontendError {
    let diagnostic = make_diagnostic(
        SYNTAX_BODY_MISSING,
        "syntax definition has no body".to_owned(),
        vec![Label {
            style: LabelStyle::Primary,
            span,
            message: "expected a type body".to_owned(),
        }],
    );
    Box::new(diagnostic.into())
}

const SYNTAX_IDENTIFIER_MISSING: &str = "parse/syntax-identifier-missing";

/// Reports a syntax declaration without identifiers.
pub(crate) fn syntax_identifier_missing(span: Span) -> FrontendError {
    let diagnostic = make_diagnostic(
        SYNTAX_IDENTIFIER_MISSING,
        "empty syntax declaration".to_owned(),
        vec![Label {
            style: LabelStyle::Primary,
            span,
            message: "expected at least one identifier".to_owned(),
        }],
    );
    Box::new(diagnostic.into())
}
