use std::path::Path;

use p4spec_rust::{
    frontend::{
        error::{FrontendError, LexErrorKind, SyntaxErrorKind},
        lexer::Lexer,
        parse::parse_files,
    },
    lang::common::source::{Position, Span},
};

fn span(file: &str, col_l: usize, col_r: usize) -> Span {
    Span::new(Position::new(file, 1, col_l), Position::new(file, 1, col_r))
}

#[test]
fn test_lexical_errors_convert_without_losing_category_or_span() {
    let lexical = Lexer::new("source.watsup", "@", |_| false)
        .next()
        .unwrap()
        .unwrap_err();
    let expected_span = span("source.watsup", 0, 1);

    assert_eq!(lexical.node, LexErrorKind::MalformedToken);
    assert_eq!(lexical.span, expected_span);

    let error = FrontendError::from(lexical);

    assert!(matches!(
        &error,
        FrontendError::Lexical(error)
            if error.node == LexErrorKind::MalformedToken && error.span == expected_span
    ));
    assert_eq!(error.to_string(), "malformed token at source.watsup:1.1-1.2");
}

#[test]
fn test_syntax_failures_report_typed_kinds_and_precise_spans() {
    use SyntaxErrorKind::{
        EmptyStructType, EmptySyntaxDeclaration, EmptyType, EmptyVariantType, ExpectedNotationType,
        HintsInPlainTypeDefinition, UnexpectedToken,
    };

    let fixtures = [
        ("empty-struct-type", EmptyStructType, (1, 15), (1, 17)),
        ("empty-syntax-declaration", EmptySyntaxDeclaration, (1, 0), (1, 6)),
        ("empty-type", EmptyType, (1, 14), (1, 14)),
        ("empty-variant-type", EmptyVariantType, (1, 15), (1, 16)),
        ("expected-notation-type", ExpectedNotationType, (1, 13), (1, 16)),
        ("hints-in-plain-type", HintsInPlainTypeDefinition, (1, 15), (1, 28)),
        ("relation-in-argument", UnexpectedToken, (1, 19), (1, 21)),
        ("relation-in-index", UnexpectedToken, (1, 18), (1, 19)),
        ("relation-in-list", UnexpectedToken, (1, 17), (1, 19)),
        ("relation-in-table-body", UnexpectedToken, (2, 9), (2, 11)),
        ("relation-in-tuple", UnexpectedToken, (1, 18), (1, 20)),
        // The lexer emits an explicit EOF token for the incomplete definition
        ("unexpected-end", UnexpectedToken, (2, 0), (2, 0)),
    ];
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/frontend/negative");
    for (name, kind_expect, (line_l, col_l), (line_r, col_r)) in fixtures {
        let path = dir.join(format!("{name}.watsup"));
        let error = parse_files([&path]).expect_err(name);
        let FrontendError::Syntax(error) = error else {
            panic!("{name}: expected syntax error, got {error}");
        };
        let file = path.to_string_lossy();
        let span_expect = Span::new(
            Position::new(file.as_ref(), line_l, col_l),
            Position::new(file.as_ref(), line_r, col_r),
        );

        assert_eq!(error.node, kind_expect, "{name}");
        assert_eq!(error.span, span_expect, "{name}");
    }
}
