use std::rc::Rc;

use p4spec_rust::{
    interface::p4::{
        context::Context,
        lexer::{Lexer, Token},
    },
    runtime::value::get,
};

fn tokens(source: &str, context: Rc<Context>) -> Vec<Token> {
    Lexer::new(Rc::from("input.p4"), source, context)
        .map(|token| token.unwrap().node)
        .collect()
}

#[test]
fn test_identifiers_are_followed_by_context_sensitive_classification() {
    let context = Rc::new(Context::new());
    context.declare_type("Header", true).unwrap();
    let tokens = tokens("Header<bit<8>> value", context);

    assert!(matches!(&tokens[0], Token::Name(value) if get::text(value) == Ok("Header")));
    assert_eq!(tokens[1], Token::TypeName);
    assert_eq!(tokens[2], Token::LeftAngleArgs);
    assert_eq!(tokens[3], Token::Bit);
    assert_eq!(tokens[4], Token::LeftAngleArgs);
    assert!(matches!(&tokens[5], Token::NumberInt(_, lexeme) if lexeme == "8"));
    assert_eq!(tokens[6], Token::RightAngle);
    assert_eq!(tokens[7], Token::RightAngleShift);
    assert!(matches!(&tokens[8], Token::Name(value) if get::text(value) == Ok("value")));
    assert_eq!(tokens[9], Token::Identifier);
    assert_eq!(tokens[10], Token::End);
}

#[test]
fn test_lexer_preserves_string_escapes_and_preprocessor_locations() {
    let context = Rc::new(Context::new());
    let mut lexer = Lexer::new(
        Rc::from("preprocessed.p4"),
        "# 42 \"original.p4\"\n\"a\\n\\\"b\"",
        context,
    );
    let token = lexer.next().unwrap().unwrap();

    assert!(matches!(&token.node, Token::StringLiteral(value) if get::text(value) == Ok("a\n\"b")));
    assert_eq!(token.span.left.file.as_ref(), "original.p4");
    assert_eq!(token.span.left.line, 42);
}

#[test]
fn test_comments_are_skipped_and_unsupported_escapes_are_located_errors() {
    let context = Rc::new(Context::new());
    assert_eq!(
        tokens(
            "/* block\n comment */ true // tail\nfalse",
            Rc::clone(&context)
        ),
        [Token::True, Token::False, Token::End]
    );

    let error = Lexer::new(Rc::from("bad.p4"), "\"\\t\"", context)
        .next()
        .unwrap()
        .unwrap_err();
    assert_eq!(error.span.left.file.as_ref(), "bad.p4");
    assert!(matches!(
        error.kind,
        p4spec_rust::interface::p4::error::P4ErrorKind::Lex(_)
    ));
}

#[test]
fn test_shift_and_type_constructor_angles_are_distinct() {
    let context = Rc::new(Context::new());
    context.declare_type("Header", true).unwrap();

    let tokens = tokens("Header<bit<8>>(x); x >> 1", context);

    assert_eq!(tokens[1], Token::TypeNameExpression);
    assert_eq!(tokens[7], Token::RightAngleShift);
    assert!(tokens.contains(&Token::ShiftRight));
}
