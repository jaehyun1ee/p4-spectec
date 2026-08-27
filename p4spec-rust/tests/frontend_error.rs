use p4spec_rust::{
    frontend::{
        error::{FrontendError, LexErrorKind},
        lexer::Lexer,
    },
    lang::common::source::{Position, Span},
};

fn span(file: &str, left: i64, right: i64) -> Span {
    Span::new(Position::new(file, 1, left), Position::new(file, 1, right))
}

#[test]
fn lexical_errors_convert_without_losing_category_or_span() {
    let lexical = Lexer::new("source.watsup", "@")
        .next()
        .unwrap()
        .unwrap_err();
    let expected_span = span("source.watsup", 0, 1);

    assert_eq!(lexical.kind, LexErrorKind::MalformedToken);
    assert_eq!(lexical.span, expected_span);

    let error = FrontendError::from(lexical);

    assert_eq!(error.span(), &expected_span);
    assert_eq!(
        error.to_string(),
        "malformed token at source.watsup:1.1-1.2"
    );
    assert!(matches!(
        error,
        FrontendError::Lexical(error) if error.kind == LexErrorKind::MalformedToken
    ));
}
