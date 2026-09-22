use std::cell::Cell;

use p4spec_rust::{
    frontend::lexer::{Lexer, Token},
    lang::{common::prim::num::Natural, common::source::Position},
};

fn token_nodes(source: &str) -> Vec<Token> {
    Lexer::new("lexer-test.watsup", source, |_| false)
        .map(|result| result.expect("valid lexer fixture").node)
        .collect()
}

#[test]
fn test_fixed_lexemes_map_to_their_grammar_tokens() {
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
fn test_literals_and_identifiers_preserve_payloads() {
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
fn test_byte_escapes_decode_valid_utf8_sequences() {
    assert_eq!(token_nodes("\"\\C3\\A9\""), vec![Token::TextLiteral("é".to_owned()), Token::Eof]);
}

#[test]
fn test_numbered_holes_accept_large_values() {
    assert_eq!(
        token_nodes("%4611686018427387904"),
        vec![Token::NumberedHole(4_611_686_018_427_387_904), Token::Eof]
    );
}

#[test]
fn test_byte_escapes_reject_non_utf8_text() {
    let source = "\"\\FF\"";
    let error = Lexer::new("unicode-policy.watsup", source, |_| false)
        .next()
        .expect("lexer result")
        .expect_err("byte-only text");

    assert_eq!(crate::cause(&error).code.as_deref().unwrap(), "parse/text-encoding-invalid");
    assert_eq!(
        crate::cause(&error).labels[0].span.left,
        Position::new("unicode-policy.watsup", 1, 0)
    );
    assert_eq!(
        crate::cause(&error).labels[0].span.right,
        Position::new("unicode-policy.watsup", 1, source.len())
    );
}

#[test]
fn test_unicode_escapes_reject_surrogates() {
    let error = Lexer::new("unicode-policy.watsup", "\"\\u{D800}\"", |_| false)
        .next()
        .expect("lexer result")
        .expect_err("surrogate escape");

    assert_eq!(
        crate::cause(&error).code.as_deref().unwrap(),
        "parse/text-escape-codepoint-invalid"
    );
    assert_eq!(
        crate::cause(&error).labels[0].span.left,
        Position::new("unicode-policy.watsup", 1, 1)
    );
    assert_eq!(
        crate::cause(&error).labels[0].span.right,
        Position::new("unicode-policy.watsup", 1, 9)
    );
}

#[test]
fn test_comments_and_newlines_emit_only_significant_layout_tokens() {
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
fn test_lexemes_carry_byte_based_source_positions() {
    let lexemes =
        Lexer::new("source.watsup", "A\n  | \"é\"", |_| false).collect::<Result<Vec<_>, _>>();
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
fn test_uppercase_identifier_classification_is_lazy_and_contextual() {
    let classifier_calls = Cell::new(0);
    let mut lexer = Lexer::new("scope.watsup", "Bound Next", |id| {
        classifier_calls.set(classifier_calls.get() + 1);
        id == "Bound"
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
fn test_lexical_failures_report_codes_and_precise_spans() {
    let fixtures = [
        ("\"unterminated", "parse/text-literal-incomplete", 13, 13),
        ("\"abc\\", "parse/text-literal-incomplete", 5, 5),
        ("\"bad\\q\"", "parse/text-escape-invalid", 4, 6),
        ("\"bad\u{7}\"", "parse/text-character-invalid", 4, 4),
        ("\"unterminated\nnext", "parse/text-literal-incomplete", 13, 13),
        ("(; unclosed", "parse/block-comment-incomplete", 11, 11),
        ("@", "parse/character-invalid", 0, 1),
        ("é", "parse/character-invalid", 0, 2),
        ("\u{7}", "parse/character-invalid", 0, 1),
        ("%999999999999999999999999", "parse/hole-index-out-of-bounds", 0, 25),
    ];

    for (source, kind, left_column, right_column) in fixtures {
        let error = Lexer::new("error.watsup", source, |_| false)
            .next()
            .expect("lexer result")
            .expect_err("invalid source");

        assert_eq!(crate::cause(&error).code.as_deref().unwrap(), kind, "source: {source:?}");
        assert_eq!(
            crate::cause(&error).labels[0].span.left,
            Position::new("error.watsup", 1, left_column)
        );
        assert_eq!(
            crate::cause(&error).labels[0].span.right,
            Position::new("error.watsup", 1, right_column),
            "source: {source:?}"
        );
    }
}

#[test]
fn test_trailing_backslash_reports_a_renderable_eof_span() {
    use p4spec_rust::diagnostic::{RenderConfig, Renderer};

    for (source, line, column) in [("\"abc\\", 1, 5), ("\n\"é\\", 2, 4)] {
        let report = Lexer::new("escape.watsup", source, |_| false)
            .next()
            .unwrap()
            .unwrap_err();
        assert_eq!(crate::cause(&report).code.as_deref(), Some("parse/text-literal-incomplete"));
        let pos = Position::new("escape.watsup", line, column);
        assert_eq!(crate::cause(&report).labels[0].span.left, pos);
        assert_eq!(crate::cause(&report).labels[0].span.right, pos);

        let mut renderer = Renderer::new(RenderConfig::default());
        renderer.insert_source("escape.watsup", source);
        let text = renderer
            .render_to_string(&report)
            .expect("valid EOF position");
        assert!(text.contains("expected a closing quote"), "{text}");
    }
}

#[test]
fn test_escaped_newline_reports_a_renderable_multiline_span() {
    use p4spec_rust::diagnostic::{RenderConfig, Renderer};

    let source = "\"abc\\\n";
    let report = Lexer::new("escape.watsup", source, |_| false)
        .next()
        .unwrap()
        .unwrap_err();
    assert_eq!(crate::cause(&report).code.as_deref(), Some("parse/text-escape-invalid"));
    assert_eq!(crate::cause(&report).labels[0].span.left, Position::new("escape.watsup", 1, 4));
    assert_eq!(crate::cause(&report).labels[0].span.right, Position::new("escape.watsup", 2, 0));

    let mut renderer = Renderer::new(RenderConfig::default());
    renderer.insert_source("escape.watsup", source);
    let text = renderer
        .render_to_string(&report)
        .expect("valid byte endpoints");
    assert!(text.contains("invalid escape"));
}

#[test]
fn test_unicode_escape_diagnostics_distinguish_invalid_scalar_values() {
    for (digits, reason) in [
        ("D800", "surrogate"),
        ("DFFF", "surrogate"),
        ("110000", "maximum"),
        ("FFFFFFFFFFFFFFFF", "maximum"),
    ] {
        let source = format!("\"\\u{{{digits}}}\"");
        let report = Lexer::new("escape.watsup", &source, |_| false)
            .next()
            .unwrap()
            .unwrap_err();
        assert_eq!(
            crate::cause(&report).code.as_deref(),
            Some("parse/text-escape-codepoint-invalid")
        );
        assert!(
            crate::cause(&report).message.contains(digits),
            "{}",
            crate::cause(&report).message
        );
        assert!(
            crate::cause(&report).message.contains(reason),
            "{}",
            crate::cause(&report).message
        );
        assert!(
            crate::cause(&report)
                .notes
                .iter()
                .any(|text| text.contains("U+0000")
                    && text.contains("U+D7FF")
                    && text.contains("U+E000")
                    && text.contains("U+10FFFF"))
        );
    }
}

#[test]
fn test_invalid_characters_name_controls_without_emitting_them() {
    for source in ["\u{b}", "\"\u{b}\""] {
        let report = Lexer::new("control.watsup", source, |_| false)
            .next()
            .unwrap()
            .unwrap_err();
        assert!(crate::cause(&report).message.contains("U+000B"));
        assert!(crate::cause(&report).message.contains("vertical tab"));
        assert!(!crate::cause(&report).message.contains('\u{b}'));
        if source.starts_with('"') {
            assert!(
                crate::cause(&report)
                    .notes
                    .iter()
                    .any(|text| text.contains("\\u{B}"))
            );
        }
    }
}

#[test]
fn test_invalid_escape_names_escape_and_supported_forms() {
    let report = Lexer::new("escape.watsup", r#""\q""#, |_| false)
        .next()
        .unwrap()
        .unwrap_err();
    assert!(crate::cause(&report).message.contains("\\q"));
    assert!(
        crate::cause(&report)
            .notes
            .iter()
            .any(|text| text.contains("\\n") && text.contains("\\HH") && text.contains("\\u{HEX}"))
    );
}

#[test]
fn test_decoded_utf8_error_identifies_escaped_bytes_and_offset() {
    let report = Lexer::new("escape.watsup", r#""a\FF""#, |_| false)
        .next()
        .unwrap()
        .unwrap_err();
    assert!(crate::cause(&report).message.contains("decoded bytes"));
    assert!(crate::cause(&report).labels[0].message.contains("0xFF"));
    assert!(crate::cause(&report).labels[0].message.contains("offset 1"));
    assert!(
        crate::cause(&report)
            .notes
            .iter()
            .any(|text| text.contains("\\u{FF}"))
    );
}

#[test]
fn test_unclosed_comment_labels_only_still_open_delimiters() {
    use p4spec_rust::diagnostic::LabelStyle;

    for (source, columns) in [
        ("(; outer (; inner", vec![0, 9]),
        ("(; outer (; closed ;) (; inner", vec![0, 22]),
        ("(; outer (; closed ;)", vec![0]),
    ] {
        let report = Lexer::new("comment.watsup", source, |_| false)
            .next()
            .unwrap()
            .unwrap_err();
        assert_eq!(crate::cause(&report).labels[0].style, LabelStyle::Primary);
        assert_eq!(crate::cause(&report).labels[0].span.left.column, source.len());
        assert_eq!(crate::cause(&report).labels.len(), columns.len() + 1);
        for (label, column) in crate::cause(&report).labels[1..].iter().zip(columns) {
            assert_eq!(label.style, LabelStyle::Secondary);
            assert_eq!(label.message, "comment opened here");
            assert_eq!(label.span.left.column, column);
            assert_eq!(label.span.right.column, column + 2);
        }
    }
}

#[test]
fn test_unicode_escape_scalar_boundaries_remain_accepted() {
    assert_eq!(
        token_nodes(r#""\u{D7FF}\u{E000}\u{10FFFF}""#),
        vec![Token::TextLiteral("\u{D7FF}\u{E000}\u{10FFFF}".to_owned()), Token::Eof]
    );
}
