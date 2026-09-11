use p4spec_rust::lang::data::value::ValueArena;
use std::rc::Rc;

use p4spec_rust::{
    interface::p4::{
        context::Context,
        lexer::{Lexer, Token},
    },
    lang::data::value::get,
};

fn tokens(source: &str, context: Rc<Context>) -> Vec<Token> {
    Lexer::new(Rc::from("input.p4"), source, Rc::clone(&context))
        .map(|token| token.unwrap().node)
        .collect()
}

#[test]
fn test_identifiers_are_followed_by_context_sensitive_classification() {
    let mut arena = ValueArena::new();
    let context = Rc::new(Context::new(&mut arena));
    context.declare_typ("Header", true).unwrap();
    let tokens = tokens("Header<bit<8>> value", Rc::clone(&context));

    assert!(
        matches!(&tokens[0], Token::Name(value) if get::text(&context.arena(), value) == Ok("Header"))
    );
    assert_eq!(tokens[1], Token::TypeName);
    assert_eq!(tokens[2], Token::LeftAngleArgs);
    assert_eq!(tokens[3], Token::Bit);
    assert_eq!(tokens[4], Token::LeftAngleArgs);
    assert!(matches!(&tokens[5], Token::NumberInt(_, lexeme) if lexeme == "8"));
    assert_eq!(tokens[6], Token::RightAngle);
    assert_eq!(tokens[7], Token::RightAngleShift);
    assert!(
        matches!(&tokens[8], Token::Name(value) if get::text(&context.arena(), value) == Ok("value"))
    );
    assert_eq!(tokens[9], Token::Identifier);
    assert_eq!(tokens[10], Token::End);
}

#[test]
fn test_lexer_preserves_string_escapes_and_preprocessor_locations() {
    let mut arena = ValueArena::new();
    let context = Rc::new(Context::new(&mut arena));
    let mut lexer = Lexer::new(
        Rc::from("preprocessed.p4"),
        "# 42 \"original.p4\"\n\"a\\n\\\"b\"",
        Rc::clone(&context),
    );
    let token = lexer.next().unwrap().unwrap();

    assert!(
        matches!(&token.node, Token::StringLiteral(value) if get::text(&context.arena(), value) == Ok("a\n\"b"))
    );
    assert_eq!(token.span.left.file.as_ref(), "original.p4");
    assert_eq!(token.span.left.line, 42);
}

#[test]
fn test_preprocessor_preserves_paths_containing_spaces() {
    let mut arena = ValueArena::new();
    let context = Rc::new(Context::new(&mut arena));
    let token = Lexer::new(
        Rc::from("preprocessed.p4"),
        "# 42 \"dir/my file.p4\" 2\ntrue",
        Rc::clone(&context),
    )
    .next()
    .unwrap()
    .unwrap();

    assert_eq!(token.node, Token::True);
    assert_eq!(token.span.left.file.as_ref(), "dir/my file.p4");
    assert_eq!(token.span.left.line, 42);
}

#[test]
fn test_comments_are_skipped_and_unsupported_escapes_are_located_errors() {
    let mut arena = ValueArena::new();
    let context = Rc::new(Context::new(&mut arena));
    assert_eq!(
        tokens(
            "/* block\n comment */ true // tail\nfalse",
            Rc::clone(&context)
        ),
        [Token::True, Token::False, Token::End]
    );

    let error = Lexer::new(Rc::from("bad.p4"), "\"\\t\"", Rc::clone(&context))
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
    let mut arena = ValueArena::new();
    let context = Rc::new(Context::new(&mut arena));
    context.declare_typ("Header", true).unwrap();

    let tokens = tokens("Header<bit<8>>(x); x >> 1", Rc::clone(&context));

    assert_eq!(tokens[1], Token::TypeNameExpression);
    assert_eq!(tokens[7], Token::RightAngleShift);
    assert!(
        tokens
            .windows(2)
            .any(|tokens| tokens == [Token::RightAngle, Token::RightAngleShift])
    );
}

#[test]
fn test_right_shift_uses_source_two_token_stream() {
    let mut arena = ValueArena::new();
    let tokens = tokens("x >> 1", Rc::new(Context::new(&mut arena)));

    assert!(matches!(&tokens[0], Token::Name(value) if get::text(&arena, value) == Ok("x")));
    assert_eq!(tokens[1], Token::Identifier);
    assert_eq!(tokens[2], Token::RightAngle);
    assert_eq!(tokens[3], Token::RightAngleShift);
    assert!(matches!(&tokens[4], Token::NumberInt(_, lexeme) if lexeme == "1"));
    assert_eq!(tokens[5], Token::End);
}

#[test]
fn test_numbers_end_at_their_lexical_boundary() {
    let mut arena = ValueArena::new();
    let context = Rc::new(Context::new(&mut arena));
    let tokens = tokens("123abc 0b102 8w3foo", Rc::clone(&context));

    assert!(matches!(&tokens[0], Token::NumberInt(_, lexeme) if lexeme == "123"));
    assert!(
        matches!(&tokens[1], Token::Name(value) if get::text(&context.arena(), value) == Ok("abc"))
    );
    assert_eq!(tokens[2], Token::Identifier);
    assert!(matches!(&tokens[3], Token::NumberInt(_, lexeme) if lexeme == "0b10"));
    assert!(matches!(&tokens[4], Token::NumberInt(_, lexeme) if lexeme == "2"));
    assert!(matches!(&tokens[5], Token::Number(_, lexeme) if lexeme == "3"));
    assert!(
        matches!(&tokens[6], Token::Name(value) if get::text(&context.arena(), value) == Ok("foo"))
    );
    assert_eq!(tokens[7], Token::Identifier);
    assert_eq!(tokens[8], Token::End);
}

#[test]
fn test_fixed_tokens_use_maximal_munch_in_grammar_order() {
    let mut arena = ValueArena::new();
    let context = Rc::new(Context::new(&mut arena));
    let tokens = tokens("+ += |+| |+|= . .. ... > >= >>= >>", Rc::clone(&context));

    assert_eq!(
        tokens,
        [
            Token::Plus,
            Token::PlusAssign,
            Token::PlusSaturating,
            Token::PlusSaturatingAssign,
            Token::Dot,
            Token::Range,
            Token::Dots,
            Token::RightAngle,
            Token::GreaterEqual,
            Token::ShiftRightAssign,
            Token::RightAngle,
            Token::RightAngleShift,
            Token::End,
        ]
    );
}

#[test]
fn test_string_token_uses_closing_quote_but_payload_spans_the_literal() {
    let mut arena = ValueArena::new();
    for literal in ["\"\"", "\"text\"", "\"a\\\"b\\n\\\\\"", "\"a\nb\""] {
        let source = format!("  {literal}");
        let token = Lexer::new(
            Rc::from("string.p4"),
            &source,
            Rc::new(Context::new(&mut arena)),
        )
        .next()
        .unwrap()
        .unwrap();
        let Token::StringLiteral(value) = token.node else {
            panic!("string literal")
        };
        assert_eq!(token.span.left.line, 1);
        assert_eq!(token.span.left.column, source.len() as i64 - 1);
        assert_eq!(token.span.right.column, source.len() as i64);
        assert_eq!(arena.span(&value).left.column, 2);
        assert_eq!(arena.span(&value).right, token.span.right);
    }
}

#[test]
fn test_string_failures_locate_the_escape_or_end_of_input() {
    let mut arena = ValueArena::new();
    for (source, left, right) in [("\"ab\\t\"", 3, 5), ("\"ab", 3, 3), ("\"ab\\", 4, 4)] {
        let error = Lexer::new(
            Rc::from("string.p4"),
            source,
            Rc::new(Context::new(&mut arena)),
        )
        .next()
        .unwrap()
        .unwrap_err();
        assert_eq!((error.span.left.line, error.span.left.column), (1, left));
        assert_eq!((error.span.right.line, error.span.right.column), (1, right));
    }
}
