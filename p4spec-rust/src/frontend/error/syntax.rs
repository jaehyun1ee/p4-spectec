//! Diagnostics for parser expectations and grammar constraints
//!
//! Constructors retain the parser's responsible locations,
//! using token vocabulary descriptions for expected alternatives.

use crate::diagnostic::{Label, LabelStyle};
use crate::frontend::tokens::describe_expected;
use crate::lang::common::source::Span;

use super::{FrontendError, make_report};

// = Token expectations

const TOKEN_INVALID: &str = "parse/token-invalid";

/// Reports an unexpected token.
pub(crate) fn token_invalid(
    span: Span,
    actual: Option<&str>,
    expected: &[String],
) -> FrontendError {
    make_report(
        TOKEN_INVALID,
        actual
            .map_or_else(|| "unexpected token".to_owned(), |actual| format!("unexpected {actual}")),
        vec![Label {
            style: LabelStyle::Primary,
            span,
            message: describe_expected(expected).unwrap_or_else(|| "unexpected token".to_owned()),
        }],
    )
}

const INPUT_INCOMPLETE: &str = "parse/input-incomplete";

/// Reports an unexpected end of input with the grammar's expected alternatives.
pub(crate) fn input_incomplete(span: Span, expected: &[String]) -> FrontendError {
    make_report(
        INPUT_INCOMPLETE,
        "unexpected end of input".to_owned(),
        vec![Label {
            style: LabelStyle::Primary,
            span,
            message: describe_expected(expected)
                .unwrap_or_else(|| "expected more input".to_owned()),
        }],
    )
}

// = Relation signatures

const RELATION_SIGNATURE_INVALID: &str = "parse/relation-signature-invalid";

/// Reports a plain type used as a relation signature.
pub(crate) fn relation_signature_invalid(span: Span) -> FrontendError {
    let mut report = make_report(
        RELATION_SIGNATURE_INVALID,
        "relation signature must be a notation type".to_owned(),
        vec![Label {
            style: LabelStyle::Primary,
            span,
            message: "expected a notation type".to_owned(),
        }],
    );
    report.notes.push("A notation type includes literal tokens like `|-` or `:` that rules pattern-match against. A bare type like `nat` names a set of values without any tokens, so it cannot serve as a relation signature.".to_owned());
    report
}

// = Type definitions

const STRUCT_FIELD_MISSING: &str = "parse/struct-field-missing";

/// Reports a struct type without fields.
pub(crate) fn struct_field_missing(span: Span) -> FrontendError {
    make_report(
        STRUCT_FIELD_MISSING,
        "empty struct type".to_owned(),
        vec![Label {
            style: LabelStyle::Primary,
            span,
            message: "expected at least one field".to_owned(),
        }],
    )
}

const VARIANT_CASE_MISSING: &str = "parse/variant-case-missing";

/// Reports a variant type without cases.
pub(crate) fn variant_case_missing(span: Span) -> FrontendError {
    make_report(
        VARIANT_CASE_MISSING,
        "empty variant type".to_owned(),
        vec![Label {
            style: LabelStyle::Primary,
            span,
            message: "expected at least one case".to_owned(),
        }],
    )
}

const PLAIN_TYPE_HINT_UNSUPPORTED: &str = "parse/plain-type-hint-unsupported";

/// Reports hints attached to a plain type definition.
pub(crate) fn plain_type_hint_unsupported(span: Span) -> FrontendError {
    let mut report = make_report(
        PLAIN_TYPE_HINT_UNSUPPORTED,
        "hints are not allowed on a plain type definition".to_owned(),
        vec![Label {
            style: LabelStyle::Primary,
            span,
            message: "plain types inherit their hints".to_owned(),
        }],
    );
    report.notes.push("A plain type definition aliases an existing type, as in `syntax x = nat`. It inherits the aliased type's hints and cannot declare its own.".to_owned());
    report
}

// = Syntax declarations

const SYNTAX_BODY_MISSING: &str = "parse/syntax-body-missing";

/// Reports a syntax definition without a body.
pub(crate) fn syntax_body_missing(span: Span) -> FrontendError {
    make_report(
        SYNTAX_BODY_MISSING,
        "syntax definition has no body".to_owned(),
        vec![Label {
            style: LabelStyle::Primary,
            span,
            message: "expected a type body".to_owned(),
        }],
    )
}

const SYNTAX_IDENTIFIER_MISSING: &str = "parse/syntax-identifier-missing";

/// Reports a syntax declaration without identifiers.
pub(crate) fn syntax_identifier_missing(span: Span) -> FrontendError {
    make_report(
        SYNTAX_IDENTIFIER_MISSING,
        "empty syntax declaration".to_owned(),
        vec![Label {
            style: LabelStyle::Primary,
            span,
            message: "expected at least one identifier".to_owned(),
        }],
    )
}
