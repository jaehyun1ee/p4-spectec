//! Parser inputs for diagnostic snapshot acceptance
//!
//! Fixture names and constructed cases run in the pinned reference order.
//! Rendered output is compared with `expected/diagnostic/parse.expected`.

/// Pins the OCaml inputs represented by the parser cases.
pub const REVISION: &str = "960e2922b55288722c413732002e33ac06664e6f";

/// Lists file fixtures and constructed parser inputs in snapshot order.
pub const PARSE: &[&str] = &[
    "parse-hint-on-plain.watsup",
    "parse-hole-index-out-of-range.watsup",
    "parse-illegal-escape.watsup",
    "parse-relation-body-must-be-notation.watsup",
    "parse-stray-non-ascii-char.watsup",
    "parse-stray-printable.watsup",
    "parse-stray-rbrace.watsup",
    "parse-struct-no-fields.watsup",
    "parse-syntax-empty-body.watsup",
    "parse-syntax-no-ids.watsup",
    "parse-unclosed-block-comment.watsup",
    "parse-unclosed-text-literal.watsup",
    "parse-variant-bar-no-cases.watsup",
    "parse-io-error",
    "parse-illegal-control-in-text-literal",
    "parse-malformed-mixop",
    "parse-malformed-utf8",
    "parse-malformed-utf8-in-comment",
    "parse-misplaced-control-char",
    "parse-directory-io-error",
];
