use std::path::Path;

use p4spec_rust::{
    frontend::{lexer::Lexer, parse::parse_files},
    lang::common::source::{Position, Span},
};

fn span(file: &str, col_l: usize, col_r: usize) -> Span {
    Span::new(Position::new(file, 1, col_l), Position::new(file, 1, col_r))
}

#[test]
fn test_lexical_reports_preserve_code_and_span() {
    let lexical = Lexer::new("source.watsup", "@", |_| false)
        .next()
        .unwrap()
        .unwrap_err();
    let expected_span = span("source.watsup", 0, 1);

    assert_eq!(lexical.code.as_deref(), Some("parse/character-invalid"));
    assert_eq!(lexical.labels[0].span, expected_span);
    assert_eq!(lexical.source, "parse");
}

#[test]
fn test_syntax_failures_report_codes_and_precise_spans() {
    let fixtures = [
        ("empty-struct-type", "parse/struct-field-missing", (1, 15), (1, 17)),
        ("empty-syntax-declaration", "parse/syntax-identifier-missing", (1, 0), (1, 6)),
        ("empty-type", "parse/syntax-body-missing", (1, 14), (1, 14)),
        ("empty-variant-type", "parse/variant-case-missing", (1, 15), (1, 16)),
        ("expected-notation-type", "parse/relation-signature-invalid", (1, 13), (1, 16)),
        ("hints-in-plain-type", "parse/plain-type-hint-unsupported", (1, 19), (1, 28)),
        ("relation-in-argument", "parse/token-invalid", (1, 19), (1, 21)),
        ("relation-in-index", "parse/token-invalid", (1, 18), (1, 19)),
        ("relation-in-list", "parse/token-invalid", (1, 17), (1, 19)),
        ("relation-in-table-body", "parse/token-invalid", (2, 9), (2, 11)),
        ("relation-in-tuple", "parse/token-invalid", (1, 18), (1, 20)),
        // The lexer emits an explicit EOF token for the incomplete definition
        ("unexpected-end", "parse/input-incomplete", (2, 0), (2, 0)),
    ];
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/frontend/negative");
    for (name, code_expect, (line_l, col_l), (line_r, col_r)) in fixtures {
        let path = dir.join(format!("{name}.watsup"));
        let error = parse_files([&path]).expect_err(name);
        let file = path.to_string_lossy();
        let span_expect = Span::new(
            Position::new(file.as_ref(), line_l, col_l),
            Position::new(file.as_ref(), line_r, col_r),
        );

        assert_eq!(error.code.as_deref().unwrap(), code_expect, "{name}");
        assert_eq!(error.labels[0].span, span_expect, "{name}");
    }
}
