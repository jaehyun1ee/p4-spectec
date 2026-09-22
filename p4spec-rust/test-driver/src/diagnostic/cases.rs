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

/// Lists elaboration declaration fixtures in pinned reference order.
pub const ELAB: &[&str] = &[
    "ctx-builtin-dec-redefined.watsup",
    "ctx-builtin-dec-tparam-duplicate.watsup",
    "ctx-dec-redefined.watsup",
    "ctx-dec-tparam-duplicate.watsup",
    "ctx-dec-undefined.watsup",
    "ctx-defined-dec-undefined.watsup",
    "ctx-extern-dec-redefined.watsup",
    "ctx-extern-dec-tparam-duplicate.watsup",
    "ctx-extern-relation-redefined.watsup",
    "ctx-extern-relation-rules.watsup",
    "ctx-function-otherwise-redefined.watsup",
    "ctx-metavar-id-has-suffix.watsup",
    "ctx-metavar-redefined.watsup",
    "ctx-otherwise-redefined.watsup",
    "ctx-relation-redefined.watsup",
    "ctx-relation-undefined.watsup",
    "ctx-rulegroup-redefined.watsup",
    "ctx-table-dec-redefined.watsup",
    "ctx-table-dec-undefined.watsup",
    "ctx-table-function-required.watsup",
    "ctx-table-rows-redefined.watsup",
    "ctx-type-already-defined-in-var.watsup",
    "ctx-type-fully-redefined.watsup",
    "relation-missing-rules.watsup",
    "table-function-parameter.watsup",
    "table-missing-rows.watsup",
    "table-non-bool-return.watsup",
    "type-dec-missing-clauses.watsup",
];

/// Counts elaboration cases assigned to D04 but not registered yet.
pub const ELAB_D04_PENDING: usize = 21;

/// Counts elaboration cases assigned to D05 but not registered yet.
pub const ELAB_D05_PENDING: usize = 29;
