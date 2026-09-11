use p4spec_rust::{
    frontend::{
        error::{FrontendError, LexErrorKind},
        lexer::Lexer,
    },
    lang::common::source::{Position, Span},
};

fn span(file: &str, col_l: i64, col_r: i64) -> Span {
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
    assert_eq!(
        error.to_string(),
        "malformed token at source.watsup:1.1-1.2"
    );
}
