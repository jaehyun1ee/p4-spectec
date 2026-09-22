//! Diagnostics for characters, text literals, holes, and comments
//!
//! Lexer sites supply responsible spans and invalid input details.
//! Character descriptions remain local to their lexical diagnostics.

use crate::diagnostic::{Label, LabelStyle};
use crate::lang::common::source::Span;

use super::{LexError, describe_utf8_error, make_report};

// = Helpers

/// Names control characters without putting them into diagnostic output.
fn describe_character(character: char) -> String {
    let name = match character {
        '\0' => "null",
        '\u{7}' => "bell",
        '\u{8}' => "backspace",
        '\t' => "horizontal tab",
        '\n' => "line feed",
        '\u{b}' => "vertical tab",
        '\u{c}' => "form feed",
        '\r' => "carriage return",
        '\u{1b}' => "escape",
        '\u{7f}' => "delete",
        _ if character.is_control() => "control character",
        _ => return format!("character {character:?} (U+{:04X})", u32::from(character)),
    };
    format!("U+{:04X} {name}", u32::from(character))
}

// = Text literals

const TEXT_LITERAL_INCOMPLETE: &str = "parse/text-literal-incomplete";

/// Reports an unclosed text literal.
pub(crate) fn text_literal_incomplete(span: Span) -> LexError {
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
pub(crate) fn text_character_invalid(span: Span, character: char) -> LexError {
    let mut report = make_report(
        TEXT_CHARACTER_INVALID,
        format!("{} is not allowed literally in a text literal", describe_character(character)),
        vec![Label {
            style: LabelStyle::Primary,
            span,
            message: "escape this control character".to_owned(),
        }],
    );
    report.notes.push(format!(
        "Use `\\u{{{:X}}}` to include this character in the text.",
        u32::from(character)
    ));
    report
}

const TEXT_ESCAPE_INVALID: &str = "parse/text-escape-invalid";

/// Reports a forbidden text escape.
pub(crate) fn text_escape_invalid(span: Span, escape: &str) -> LexError {
    let mut report = make_report(
        TEXT_ESCAPE_INVALID,
        format!("escape `\\{}` is not allowed in a text literal", escape[1..].escape_debug()),
        vec![Label { style: LabelStyle::Primary, span, message: "invalid escape".to_owned() }],
    );
    report.notes.push(r#"Supported escapes are \n, \r, \t, \\, \', \", \HH (two hexadecimal digits for one byte), and \u{HEX} (a Unicode scalar value)."#.to_owned());
    report
}

const TEXT_ENCODING_INVALID: &str = "parse/text-encoding-invalid";

/// Reports decoded text bytes that are not valid UTF-8.
pub(crate) fn text_encoding_invalid(span: Span, error: &std::string::FromUtf8Error) -> LexError {
    let error_utf8 = error.utf8_error();
    let mut report = make_report(
        TEXT_ENCODING_INVALID,
        "escaped/decoded bytes in the text literal are not valid UTF-8".to_owned(),
        vec![Label {
            style: LabelStyle::Primary,
            span,
            message: format!(
                "{} at decoded byte offset {}",
                describe_utf8_error(error.as_bytes(), &error_utf8),
                error_utf8.valid_up_to()
            ),
        }],
    );
    let byte = error.as_bytes()[error_utf8.valid_up_to()];
    report.notes.push(format!("Hex escapes encode bytes, not Unicode characters. If you intended U+{byte:04X}, use `\\u{{{byte:X}}}`; otherwise supply a complete UTF-8 byte sequence."));
    report
}

const TEXT_ESCAPE_CODEPOINT_INVALID: &str = "parse/text-escape-codepoint-invalid";

/// Reports a text escape that does not encode a Unicode scalar value.
pub(crate) fn text_escape_codepoint_invalid(span: Span, digits: &str) -> LexError {
    // Surrogates fit in u32; larger values and overflow exceed Unicode's maximum
    let message = match u32::from_str_radix(digits, 16) {
        Ok(0xD800..=0xDFFF) => {
            format!("Unicode escape U+{digits} is a surrogate, not a Unicode scalar value")
        }
        _ => format!("Unicode escape U+{digits} exceeds the maximum Unicode scalar value U+10FFFF"),
    };
    let mut report = make_report(
        TEXT_ESCAPE_CODEPOINT_INVALID,
        message,
        vec![Label {
            style: LabelStyle::Primary,
            span,
            message: "expected a Unicode scalar value".to_owned(),
        }],
    );
    report.notes.push("Valid Unicode scalar values are U+0000–U+D7FF and U+E000–U+10FFFF; U+D800–U+DFFF are reserved for UTF-16 surrogates.".to_owned());
    report
}

// = Numbered holes

const HOLE_INDEX_OUT_OF_BOUNDS: &str = "parse/hole-index-out-of-bounds";

/// Reports a numbered hole outside the supported index range.
pub(crate) fn hole_index_out_of_bounds(span: Span) -> LexError {
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

// = Block comments

const BLOCK_COMMENT_INCOMPLETE: &str = "parse/block-comment-incomplete";

/// Reports an unclosed block comment.
pub(crate) fn block_comment_incomplete(span: Span, spans_open: Vec<Span>) -> LexError {
    let mut report = make_report(
        BLOCK_COMMENT_INCOMPLETE,
        "unclosed comment".to_owned(),
        vec![Label {
            style: LabelStyle::Primary,
            span,
            message: "expected a closing `;)`".to_owned(),
        }],
    );
    // Each remaining opener needs its own closing delimiter
    report
        .labels
        .extend(spans_open.into_iter().map(|span| Label {
            style: LabelStyle::Secondary,
            span,
            message: "comment opened here".to_owned(),
        }));
    report
}

// = Invalid characters

const CHARACTER_INVALID: &str = "parse/character-invalid";

/// Reports a character outside the token alphabet.
pub(crate) fn character_invalid(span: Span, character: char) -> LexError {
    make_report(
        CHARACTER_INVALID,
        format!("{} is not allowed here", describe_character(character)),
        vec![Label { style: LabelStyle::Primary, span, message: "invalid character".to_owned() }],
    )
}
