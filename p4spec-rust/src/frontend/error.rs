//! Diagnostic constructors for SpecTec source input failures
//!
//! Lexer and grammar sites select the diagnostic and responsible span.
//! File and mixfix entry points also report failures without source text.

use crate::diagnostic::{Label, LabelStyle, Report, Severity};
use crate::lang::common::source::Span;

fn make_report(code: &str, message: String, labels: Vec<Label>) -> Box<Report> {
    Box::new(Report {
        severity: Severity::Error,
        code: Some(code.to_owned()),
        message,
        labels,
        notes: Vec::new(),
        source: "parse",
        traces: Vec::new(),
    })
}

const TEXT_LITERAL_INCOMPLETE: &str = "parse/text-literal-incomplete";

/// Reports an unclosed text literal.
pub(crate) fn text_literal_incomplete(span: Span) -> Box<Report> {
    make_report(
        TEXT_LITERAL_INCOMPLETE,
        "unclosed text literal".to_owned(),
        vec![Label {
            style: LabelStyle::Primary,
            span,
            message: "expected a closing quote".to_owned(),
        }],
    )
}

const TEXT_CHARACTER_INVALID: &str = "parse/text-character-invalid";

/// Reports a forbidden control character in a text literal.
pub(crate) fn text_character_invalid(span: Span) -> Box<Report> {
    make_report(
        TEXT_CHARACTER_INVALID,
        "control character is not allowed in a text literal".to_owned(),
        vec![Label {
            style: LabelStyle::Primary,
            span,
            message: "invalid text character".to_owned(),
        }],
    )
}

const TEXT_ESCAPE_INVALID: &str = "parse/text-escape-invalid";

/// Reports a forbidden text escape.
pub(crate) fn text_escape_invalid(span: Span) -> Box<Report> {
    make_report(
        TEXT_ESCAPE_INVALID,
        "escape is not allowed in a text literal".to_owned(),
        vec![Label { style: LabelStyle::Primary, span, message: "invalid escape".to_owned() }],
    )
}

const TEXT_ENCODING_INVALID: &str = "parse/text-encoding-invalid";

/// Reports decoded text bytes that are not valid UTF-8.
pub(crate) fn text_encoding_invalid(span: Span) -> Box<Report> {
    make_report(
        TEXT_ENCODING_INVALID,
        "text literal is not valid UTF-8".to_owned(),
        vec![Label {
            style: LabelStyle::Primary,
            span,
            message: "invalid decoded bytes".to_owned(),
        }],
    )
}

const TEXT_ESCAPE_CODEPOINT_INVALID: &str = "parse/text-escape-codepoint-invalid";

/// Reports a text escape that does not encode a Unicode scalar value.
pub(crate) fn text_escape_codepoint_invalid(span: Span) -> Box<Report> {
    make_report(
        TEXT_ESCAPE_CODEPOINT_INVALID,
        "unicode escape is outside the valid codepoint range".to_owned(),
        vec![Label {
            style: LabelStyle::Primary,
            span,
            message: "expected a Unicode scalar value".to_owned(),
        }],
    )
}

const HOLE_INDEX_OUT_OF_BOUNDS: &str = "parse/hole-index-out-of-bounds";

/// Reports a numbered hole outside the supported index range.
pub(crate) fn hole_index_out_of_bounds(span: Span) -> Box<Report> {
    let mut report = make_report(
        HOLE_INDEX_OUT_OF_BOUNDS,
        "numbered hole is out of range".to_owned(),
        vec![Label {
            style: LabelStyle::Primary,
            span,
            message: "hole index exceeds the supported range".to_owned(),
        }],
    );
    report
        .notes
        .push("Use a smaller nonnegative decimal hole index.".to_owned());
    report
}

const BLOCK_COMMENT_INCOMPLETE: &str = "parse/block-comment-incomplete";

/// Reports an unclosed block comment.
pub(crate) fn block_comment_incomplete(span: Span) -> Box<Report> {
    make_report(
        BLOCK_COMMENT_INCOMPLETE,
        "unclosed comment".to_owned(),
        vec![Label {
            style: LabelStyle::Primary,
            span,
            message: "expected a closing `;)`".to_owned(),
        }],
    )
}

const CHARACTER_INVALID: &str = "parse/character-invalid";

/// Reports a character outside the token alphabet.
pub(crate) fn character_invalid(span: Span) -> Box<Report> {
    make_report(
        CHARACTER_INVALID,
        "character is not allowed here".to_owned(),
        vec![Label { style: LabelStyle::Primary, span, message: "invalid character".to_owned() }],
    )
}

const TOKEN_INVALID: &str = "parse/token-invalid";

/// Reports an unexpected token.
pub(crate) fn token_invalid(span: Span) -> Box<Report> {
    make_report(
        TOKEN_INVALID,
        "unexpected token".to_owned(),
        vec![Label { style: LabelStyle::Primary, span, message: "unexpected token".to_owned() }],
    )
}

const INPUT_INCOMPLETE: &str = "parse/input-incomplete";

/// Reports an unexpected end of input.
pub(crate) fn input_incomplete(span: Span) -> Box<Report> {
    make_report(
        INPUT_INCOMPLETE,
        "unexpected end of input".to_owned(),
        vec![Label { style: LabelStyle::Primary, span, message: "expected more input".to_owned() }],
    )
}

const RELATION_SIGNATURE_INVALID: &str = "parse/relation-signature-invalid";

/// Reports a plain type used as a relation signature.
pub(crate) fn relation_signature_invalid(span: Span) -> Box<Report> {
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

const STRUCT_FIELD_MISSING: &str = "parse/struct-field-missing";

/// Reports a struct type without fields.
pub(crate) fn struct_field_missing(span: Span) -> Box<Report> {
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
pub(crate) fn variant_case_missing(span: Span) -> Box<Report> {
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

const SYNTAX_BODY_MISSING: &str = "parse/syntax-body-missing";

/// Reports a syntax definition without a body.
pub(crate) fn syntax_body_missing(span: Span) -> Box<Report> {
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

const PLAIN_TYPE_HINT_UNSUPPORTED: &str = "parse/plain-type-hint-unsupported";

/// Reports hints attached to a plain type definition.
pub(crate) fn plain_type_hint_unsupported(span: Span) -> Box<Report> {
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

const SYNTAX_IDENTIFIER_MISSING: &str = "parse/syntax-identifier-missing";

/// Reports a syntax declaration without identifiers.
pub(crate) fn syntax_identifier_missing(span: Span) -> Box<Report> {
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

const SOURCE_ENCODING_INVALID: &str = "parse/source-encoding-invalid";

/// Reports source bytes that are not valid UTF-8.
pub(crate) fn source_encoding_invalid(span: Span) -> Box<Report> {
    make_report(
        SOURCE_ENCODING_INVALID,
        "source is not valid UTF-8".to_owned(),
        vec![Label { style: LabelStyle::Primary, span, message: "invalid UTF-8 bytes".to_owned() }],
    )
}

const COMMENT_ENCODING_INVALID: &str = "parse/comment-encoding-invalid";

/// Reports comment bytes that are not valid UTF-8.
pub(crate) fn comment_encoding_invalid(span: Span) -> Box<Report> {
    make_report(
        COMMENT_ENCODING_INVALID,
        "comment is not valid UTF-8".to_owned(),
        vec![Label { style: LabelStyle::Primary, span, message: "invalid UTF-8 bytes".to_owned() }],
    )
}

const FILE_READ_FAILED: &str = "parse/file-read-failed";
const INPUT_PATH_READ_FAILED: &str = "parse/input-path-read-failed";
const MIXFIX_OPERATOR_INVALID: &str = "parse/mixfix-operator-invalid";

/// Reports an unreadable source file at its file-only position.
pub(crate) fn file_read_failed(span: Span, error: &std::io::Error) -> Box<Report> {
    make_report(
        FILE_READ_FAILED,
        format!("I/O error: {}", error.kind()),
        vec![Label {
            style: LabelStyle::Primary,
            span,
            message: "could not read this file".to_owned(),
        }],
    )
}

/// Reports a failure to enumerate an input directory.
pub(crate) fn input_path_read_failed(
    path: &std::path::Path,
    error: &std::io::Error,
) -> Box<Report> {
    make_report(
        INPUT_PATH_READ_FAILED,
        format!("I/O error reading {}: {}", path.display(), error.kind()),
        Vec::new(),
    )
}

/// Reports a malformed runtime mixfix shape without a source location.
pub(crate) fn mixfix_operator_invalid(source: &str) -> Box<Report> {
    let message = if source.is_empty() {
        "mixfix operator must not be empty".to_owned()
    } else {
        format!("mixfix operator {source:?} is malformed")
    };
    make_report(MIXFIX_OPERATOR_INVALID, message, Vec::new())
}
