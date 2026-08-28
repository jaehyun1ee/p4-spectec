use std::cell::Cell;

use p4spec_rust::{
    frontend::{
        error::LexErrorKind,
        lexer::{Lexer, Token},
    },
    lang::{common::source::Position, xl::num::Natural},
};

fn token_nodes(source: &str) -> Vec<Token> {
    Lexer::new("lexer-test.watsup", source)
        .map(|result| result.expect("valid lexer fixture").node)
        .collect()
}

#[test]
fn fixed_lexemes_map_to_their_grammar_tokens() {
    let source = concat!(
        "`( `) `[ `] `{ `} `< `> |- -| -> ->_ => =>_ <=> ==> ~> ~>* ",
        "/\\ \\/ . .. ... , ; : :: :/ := # ## $ ? <: ~ ~~ < <- <= > >= >( ",
        "( ) [ ] { } + ++ - -- * / \\ % %12 %% !% = =/= ^ | ",
        "%latex bool nat int text syntax extern tbl relation rulegroup rule var ",
        "builtin dec def if otherwise debug hint( eps true false",
    );

    assert_eq!(
        token_nodes(source),
        vec![
            Token::TickLeftParen,
            Token::TickRightParen,
            Token::TickLeftBracket,
            Token::TickRightBracket,
            Token::TickLeftBrace,
            Token::TickRightBrace,
            Token::TickLeftAngle,
            Token::TickRightAngle,
            Token::Turnstile,
            Token::Tilesturn,
            Token::Arrow,
            Token::ArrowSub,
            Token::DoubleArrow,
            Token::DoubleArrowSub,
            Token::DoubleArrowBoth,
            Token::DoubleArrowLong,
            Token::SquigglyArrow,
            Token::SquigglyArrowStar,
            Token::And,
            Token::Or,
            Token::Dot,
            Token::DoubleDot,
            Token::TripleDot,
            Token::Comma,
            Token::Semicolon,
            Token::Colon,
            Token::DoubleColon,
            Token::ColonSlash,
            Token::ColonEquals,
            Token::Hash,
            Token::DoubleHash,
            Token::Dollar,
            Token::Question,
            Token::Subtype,
            Token::Tilde,
            Token::DoubleTilde,
            Token::LeftAngle,
            Token::LeftAngleDash,
            Token::LeftAngleEquals,
            Token::RightAngle,
            Token::RightAngleEquals,
            Token::RightAngleLeftParen,
            Token::LeftParen,
            Token::RightParen,
            Token::LeftBracket,
            Token::RightBracket,
            Token::LeftBrace,
            Token::RightBrace,
            Token::Plus,
            Token::DoublePlus,
            Token::Minus,
            Token::Dash,
            Token::Star,
            Token::Slash,
            Token::Backslash,
            Token::Hole,
            Token::NumberedHole(12),
            Token::MultipleHole,
            Token::EmptyHole,
            Token::Equals,
            Token::NotEquals,
            Token::Up,
            Token::Bar,
            Token::Latex,
            Token::Bool,
            Token::Nat,
            Token::Int,
            Token::Text,
            Token::Syntax,
            Token::Extern,
            Token::Table,
            Token::Relation,
            Token::RuleGroup,
            Token::Rule,
            Token::Var,
            Token::Builtin,
            Token::Dec,
            Token::Def,
            Token::If,
            Token::Otherwise,
            Token::Debug,
            Token::HintLeftParen,
            Token::Epsilon,
            Token::BoolLiteral(true),
            Token::BoolLiteral(false),
            Token::Eof,
        ]
    );
}

#[test]
fn literals_and_identifiers_preserve_payloads() {
    assert_eq!(
        token_nodes(
            "123_456 0xAB_CD \"line\\n\\41\\u{1F600}\" Upper lower _lower \
             Upper( lower( Upper< lower< .Field .field _Tag _Tag( _Tag< 'concrete +'",
        ),
        vec![
            Token::NaturalLiteral(Natural::from(123_456)),
            Token::HexLiteral(Natural::from(0xabcd)),
            Token::TextLiteral("line\nA😀".to_owned()),
            Token::UpperId("Upper".to_owned()),
            Token::LowerId("lower".to_owned()),
            Token::LowerId("_lower".to_owned()),
            Token::UpperIdLeftParen("Upper".to_owned()),
            Token::LowerIdLeftParen("lower".to_owned()),
            Token::UpperIdLeftAngle("Upper".to_owned()),
            Token::LowerIdLeftAngle("lower".to_owned()),
            Token::DotId("Field".to_owned()),
            Token::DotId("field".to_owned()),
            Token::TagUpperId("Tag".to_owned()),
            Token::LowerIdLeftParen("_Tag".to_owned()),
            Token::LowerIdLeftAngle("_Tag".to_owned()),
            Token::Operator("concrete +".to_owned()),
            Token::Eof,
        ]
    );
}

#[test]
fn byte_escapes_decode_valid_utf8_sequences() {
    assert_eq!(
        token_nodes("\"\\C3\\A9\""),
        vec![Token::TextLiteral("é".to_owned()), Token::Eof]
    );
}

#[test]
fn byte_escapes_reject_non_utf8_text() {
    for source in ["\"\\FF\"", "\"\\u{D800}\""] {
        let error = Lexer::new("unicode-policy.watsup", source)
            .next()
            .expect("lexer result")
            .expect_err("byte-only text");

        assert_eq!(error.node, LexErrorKind::InvalidTextEncoding);
        assert_eq!(
            error.span.left,
            Position::new("unicode-policy.watsup", 1, 0)
        );
        assert_eq!(
            error.span.right,
            Position::new("unicode-policy.watsup", 1, source.len() as i64)
        );
    }
}

#[test]
fn comments_and_newlines_emit_only_significant_layout_tokens() {
    let source = concat!(
        "one\n  | two\n\nthree\n\n\nfour,\t;; trailing\n",
        "five\\\nsix (; outer\n(; nested ;)\n;) seven",
    );

    assert_eq!(
        token_nodes(source),
        vec![
            Token::LowerId("one".to_owned()),
            Token::NewlineBar,
            Token::LowerId("two".to_owned()),
            Token::Newline2,
            Token::LowerId("three".to_owned()),
            Token::Newline3,
            Token::LowerId("four".to_owned()),
            Token::CommaNewline,
            Token::LowerId("five".to_owned()),
            Token::LowerId("six".to_owned()),
            Token::LowerId("seven".to_owned()),
            Token::Eof,
        ]
    );
}

#[test]
fn lexemes_carry_byte_based_source_positions() {
    let lexemes = Lexer::new("source.watsup", "A\n  | \"é\"").collect::<Result<Vec<_>, _>>();
    let lexemes = lexemes.expect("valid source");

    assert_eq!(
        lexemes
            .iter()
            .map(|lexeme| (&lexeme.node, &lexeme.span.left, &lexeme.span.right))
            .collect::<Vec<_>>(),
        vec![
            (
                &Token::UpperId("A".to_owned()),
                &Position::new("source.watsup", 1, 0),
                &Position::new("source.watsup", 1, 1),
            ),
            (
                &Token::NewlineBar,
                &Position::new("source.watsup", 2, 0),
                &Position::new("source.watsup", 2, 4),
            ),
            (
                &Token::TextLiteral("é".to_owned()),
                &Position::new("source.watsup", 2, 4),
                &Position::new("source.watsup", 2, 8),
            ),
            (
                &Token::Eof,
                &Position::new("source.watsup", 2, 8),
                &Position::new("source.watsup", 2, 8),
            ),
        ]
    );
}

#[test]
fn uppercase_identifier_classification_is_lazy_and_contextual() {
    let classifier_calls = Cell::new(0);
    let mut lexer = Lexer::with_uppercase_classifier("scope.watsup", "Bound Next", |identifier| {
        classifier_calls.set(classifier_calls.get() + 1);
        identifier == "Bound"
    });

    assert_eq!(classifier_calls.get(), 0);
    assert_eq!(
        lexer
            .next()
            .expect("first token")
            .expect("valid token")
            .node,
        Token::LowerId("Bound".to_owned())
    );
    assert_eq!(classifier_calls.get(), 1);
    assert_eq!(
        lexer
            .next()
            .expect("second token")
            .expect("valid token")
            .node,
        Token::UpperId("Next".to_owned())
    );
    assert_eq!(classifier_calls.get(), 2);
}

#[test]
fn lexical_failures_report_typed_kinds_and_precise_spans() {
    let fixtures = [
        ("\"unterminated", LexErrorKind::UnclosedTextLiteral, 0, 13),
        ("\"abc\\", LexErrorKind::MalformedToken, 0, 1),
        ("\"bad\\q\"", LexErrorKind::IllegalEscape, 6, 6),
        ("\"bad\u{7}\"", LexErrorKind::IllegalControlCharacter, 0, 5),
        (
            "\"unterminated\nnext",
            LexErrorKind::UnclosedTextLiteral,
            0,
            14,
        ),
        ("(; unclosed", LexErrorKind::UnclosedComment, 0, 11),
        ("@", LexErrorKind::MalformedToken, 0, 1),
        ("é", LexErrorKind::MisplacedUnicodeCharacter, 0, 2),
        ("\u{7}", LexErrorKind::MisplacedControlCharacter, 0, 1),
        (
            "%999999999999999999999999",
            LexErrorKind::HoleNumberOutOfRange,
            0,
            25,
        ),
    ];

    for (source, kind, left_column, right_column) in fixtures {
        let error = Lexer::new("error.watsup", source)
            .next()
            .expect("lexer result")
            .expect_err("invalid source");

        assert_eq!(error.node, kind, "source: {source:?}");
        assert_eq!(
            error.span.left,
            Position::new("error.watsup", 1, left_column)
        );
        assert_eq!(
            error.span.right,
            Position::new("error.watsup", 1, right_column),
            "source: {source:?}"
        );
    }
}
