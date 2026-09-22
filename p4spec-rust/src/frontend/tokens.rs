//! Contextual token adaptation between the lexer and LALRPOP
//!
//! `parser_tokens` wraps a lexer iterator in [`ParserTokens`].
//! Each `ParserTokens::next` call
//! converts source positions to [`Location`] handles
//! and forwards lexical failures as [`Report`].
//! Outside arithmetic mode it relabels `Star` as `IterStar`
//! for postfix iteration.
//!
//! `ends_sequence` and `starts_sequence` identify adjacent notation atoms.
//! If both predicates match,
//! `ParserTokens::next` returns a synthetic `Sequence`
//! and stores the real lookahead in `pending` for the following call.
//!
//! # Examples
//!
//! ```text
//! lexer:  UpperId("A"), UpperId("B")
//! parser: UpperId("A"), Sequence, UpperId("B")
//!
//! expression mode: Star -> IterStar
//! arithmetic mode: Star -> Star
//! ```

use crate::diagnostic::Report;

use crate::lang::common::source::{Phrase, Position};

use super::{
    ctx::{Context, Location},
    lexer::Token,
};

/// Wraps a lexeme stream for the parser.
pub(crate) fn parser_tokens<I>(ctx: &Context, lexemes: I) -> ParserTokens<'_, I>
where
    I: Iterator,
{
    ParserTokens { ctx, lexemes, previous_right: None, previous_token: None, pending: None }
}

/// The adapted token stream.
pub(crate) struct ParserTokens<'ctx, I: Iterator> {
    /// Parser state: modes and position interning.
    ctx: &'ctx Context,
    /// The lexer.
    lexemes: I,
    /// Where the last emitted token ended, for a `Sequence` span.
    previous_right: Option<Position>,
    /// The last emitted token, to test `ends_sequence`.
    previous_token: Option<Token>,
    /// A lexeme held back while a `Sequence` is emitted first.
    pending: Option<Phrase<Token>>,
}

/// Whether a token can begin a notation atom that follows another.
fn starts_sequence(token: &Token) -> bool {
    matches!(
        token,
        Token::TagUpperId(_)
            | Token::Operator(_)
            | Token::TickLeftParen
            | Token::TickLeftBracket
            | Token::TickLeftBrace
            | Token::TickLeftAngle
            | Token::Dollar
            | Token::DoubleHash
            | Token::LeftParen
            | Token::LeftBrace
            | Token::Hole
            | Token::NumberedHole(_)
            | Token::MultipleHole
            | Token::EmptyHole
            | Token::Latex
            | Token::Bool
            | Token::Nat
            | Token::Int
            | Token::Text
            | Token::Epsilon
            | Token::BoolLiteral(_)
            | Token::NaturalLiteral(_)
            | Token::HexLiteral(_)
            | Token::TextLiteral(_)
            | Token::UpperId(_)
            | Token::LowerId(_)
            | Token::UpperIdLeftParen(_)
    )
}

/// Whether a token can end a notation atom that another follows.
fn ends_sequence(token: &Token) -> bool {
    matches!(
        token,
        Token::TagUpperId(_)
            | Token::Operator(_)
            | Token::TickRightParen
            | Token::TickRightBracket
            | Token::TickRightBrace
            | Token::TickRightAngle
            | Token::RightParen
            | Token::RightBracket
            | Token::RightBrace
            | Token::Question
            | Token::Star
            | Token::IterStar
            | Token::Epsilon
            | Token::Bool
            | Token::Nat
            | Token::Int
            | Token::Text
            | Token::BoolLiteral(_)
            | Token::NaturalLiteral(_)
            | Token::HexLiteral(_)
            | Token::TextLiteral(_)
            | Token::UpperId(_)
            | Token::LowerId(_)
            | Token::DotId(_)
            | Token::Hole
            | Token::NumberedHole(_)
            | Token::MultipleHole
            | Token::EmptyHole
    )
}

impl<I> Iterator for ParserTokens<'_, I>
where
    I: Iterator<Item = Result<Phrase<Token>, Box<Report>>>,
{
    type Item = Result<(Location, Token, Location), Box<Report>>;

    fn next(&mut self) -> Option<Self::Item> {
        // A held-back lexeme comes before the next one from the lexer
        let mut lexeme = match self.pending.take() {
            Some(lexeme) => lexeme,
            None => match self.lexemes.next()? {
                Ok(lexeme) => lexeme,
                Err(error) => return Some(Err(error)),
            },
        };

        // `*` is iteration unless the parser is inside arithmetic
        if lexeme.node == Token::Star && !self.ctx.in_arith() {
            lexeme.node = Token::IterStar;
        }

        // Two adjacent atoms get a `Sequence` between them; the lexeme waits
        if self.previous_token.as_ref().is_some_and(ends_sequence) && starts_sequence(&lexeme.node)
        {
            let pos_l = self
                .previous_right
                .clone()
                .expect("previous token position");
            let pos_r = lexeme.span.left.clone();
            self.pending = Some(lexeme);
            self.previous_token = Some(Token::Sequence);
            self.previous_right = Some(pos_r.clone());
            return Some(Ok((self.ctx.location(pos_l), Token::Sequence, self.ctx.location(pos_r))));
        }

        // Intern both ends and remember this token for the next call
        let loc_l = self.ctx.location(lexeme.span.left);
        self.previous_right = Some(lexeme.span.right.clone());
        let loc_r = self.ctx.location(lexeme.span.right);
        self.previous_token = Some(lexeme.node.clone());
        Some(Ok((loc_l, lexeme.node, loc_r)))
    }
}

/// Presents grammar terminals as source vocabulary, grouping long alternatives.
pub(crate) fn describe_expected(expected: &[String]) -> Option<String> {
    // Remove aliases that have the same visible spelling
    let mut alternatives = Vec::new();
    for terminal in expected {
        let (category, text) = terminal_presentation(terminal.trim_matches('"'));
        if !alternatives.contains(&(category, text)) {
            alternatives.push((category, text));
        }
    }
    if alternatives.is_empty() {
        return None;
    }

    // Small sets retain every spelling; large sets show vocabulary categories
    let texts = if alternatives.len() <= 8 {
        alternatives
            .into_iter()
            .map(|(_, text)| text.to_owned())
            .collect::<Vec<_>>()
    } else {
        let mut groups: Vec<(&str, Vec<&str>)> = Vec::new();
        for (category, text) in alternatives {
            if let Some((_, texts)) = groups.iter_mut().find(|(name, _)| *name == category) {
                texts.push(text);
            } else {
                groups.push((category, vec![text]));
            }
        }
        groups
            .into_iter()
            .map(|(category, texts)| {
                if texts.len() == 1 {
                    texts[0].to_owned()
                } else if texts.len() <= 4 {
                    format!("{category} ({})", texts.join(", "))
                } else {
                    format!("{category} (such as {})", texts[..3].join(", "))
                }
            })
            .collect()
    };
    Some(format!("expected {}", texts.join(" or ")))
}

/// Translates LALRPOP terminal names into the frontend's visible vocabulary.
fn terminal_presentation(terminal: &str) -> (&str, &str) {
    match terminal {
        "TAG_UPID" => ("an identifier", "a tagged identifier"),
        "OPERATOR" => ("an operator", "a quoted operator"),
        "TICK_LPAREN" => ("a delimiter", "``(`"),
        "TICK_RPAREN" => ("a delimiter", "``)`"),
        "TICK_LBRACK" => ("a delimiter", "``[`"),
        "TICK_RBRACK" => ("a delimiter", "``]`"),
        "TICK_LBRACE" => ("a delimiter", "``{`"),
        "TICK_RBRACE" => ("a delimiter", "``}`"),
        "TICK_LANGLE" => ("a delimiter", "``<`"),
        "TICK_RANGLE" => ("a delimiter", "``>`"),
        "NL_BAR" => ("a line break", "a newline followed by `|`"),
        "NL2" => ("a line break", "a blank line"),
        "NL3" => ("a line break", "two blank lines"),
        "SEQ" => ("an adjacent token", "an adjacent token"),
        "SUB" => ("an operator or separator", "`<:`"),
        "TURNSTILE" => ("an operator or separator", "`|-`"),
        "TILESTURN" => ("an operator or separator", "`-|`"),
        "ARROW" => ("an operator or separator", "`->`"),
        "ARROW_SUB" => ("an operator or separator", "`->_`"),
        "DOUBLE_ARROW" => ("an operator or separator", "`=>`"),
        "DOUBLE_ARROW_SUB" => ("an operator or separator", "`=>_`"),
        "DOUBLE_ARROW_BOTH" => ("an operator or separator", "`<=>`"),
        "DOUBLE_ARROW_LONG" => ("an operator or separator", "`==>`"),
        "SQARROW" => ("an operator or separator", "`~>`"),
        "SQARROW_STAR" => ("an operator or separator", "`~>*`"),
        "AND" => ("an operator or separator", "`/\\`"),
        "OR" => ("an operator or separator", "`\\/`"),
        "DOT" => ("an operator or separator", "`.`"),
        "DOT2" => ("an operator or separator", "`..`"),
        "DOT3" => ("an operator or separator", "`...`"),
        "COMMA" => ("an operator or separator", "`,`"),
        "COMMA_NL" => ("a separator", "a comma followed by a newline"),
        "SEMICOLON" => ("an operator or separator", "`;`"),
        "COLON" => ("an operator or separator", "`:`"),
        "COLON2" => ("an operator or separator", "`::`"),
        "COLON_SLASH" => ("an operator or separator", "`:/`"),
        "COLON_EQ" => ("an operator or separator", "`:=`"),
        "HASH" => ("an operator or separator", "`#`"),
        "HASH2" => ("an operator or separator", "`##`"),
        "DOLLAR" => ("an operator or separator", "`$`"),
        "QUEST" => ("an operator or separator", "`?`"),
        "TILDE" => ("an operator or separator", "`~`"),
        "TILDE2" => ("an operator or separator", "`~~`"),
        "LANGLE" => ("an operator or separator", "`<`"),
        "LANGLE_DASH" => ("an operator or separator", "`<-`"),
        "LANGLE_EQ" => ("an operator or separator", "`<=`"),
        "RANGLE" => ("an operator or separator", "`>`"),
        "RANGLE_EQ" => ("an operator or separator", "`>=`"),
        "RANGLE_LPAREN" => ("a delimiter", "`>(`"),
        "LPAREN" => ("a delimiter", "`(`"),
        "RPAREN" => ("a delimiter", "`)`"),
        "LBRACK" => ("a delimiter", "`[`"),
        "RBRACK" => ("a delimiter", "`]`"),
        "LBRACE" => ("a delimiter", "`{`"),
        "RBRACE" => ("a delimiter", "`}`"),
        "PLUS" => ("an operator or separator", "`+`"),
        "PLUS2" => ("an operator or separator", "`++`"),
        "MINUS" => ("an operator or separator", "`-`"),
        "DASH" => ("an operator or separator", "`--`"),
        "STAR" => ("an operator or separator", "`*`"),
        "ITER_STAR" => ("an operator or separator", "`*`"),
        "SLASH" => ("an operator or separator", "`/`"),
        "BACKSLASH" => ("an operator or separator", "`\\`"),
        "HOLE" => ("a hole", "`%`"),
        "HOLE_NUM" => ("a hole", "a numbered hole"),
        "HOLE_MULTI" => ("a hole", "`%%`"),
        "HOLE_NIL" => ("a hole", "`!%`"),
        "EQ" => ("an operator or separator", "`=`"),
        "NEQ" => ("an operator or separator", "`=/=`"),
        "UP" => ("an operator or separator", "`^`"),
        "BAR" => ("an operator or separator", "`|`"),
        "LATEX" => ("an operator or separator", "`%latex`"),
        "BOOL" => ("a built-in type", "`bool`"),
        "NAT" => ("a built-in type", "`nat`"),
        "INT" => ("a built-in type", "`int`"),
        "TEXT" => ("a built-in type", "`text`"),
        "SYNTAX" => ("a keyword", "`syntax`"),
        "EXTERN" => ("a keyword", "`extern`"),
        "TABLE" => ("a keyword", "`tbl`"),
        "RELATION" => ("a keyword", "`relation`"),
        "RULEGROUP" => ("a keyword", "`rulegroup`"),
        "RULE" => ("a keyword", "`rule`"),
        "VAR" => ("a keyword", "`var`"),
        "BUILTIN" => ("a keyword", "`builtin`"),
        "DEC" => ("a keyword", "`dec`"),
        "DEF" => ("a keyword", "`def`"),
        "IF" => ("a keyword", "`if`"),
        "OTHERWISE" => ("a keyword", "`otherwise`"),
        "DEBUG" => ("a keyword", "`debug`"),
        "HINT_LPAREN" => ("a keyword", "`hint(`"),
        "EPS" => ("a keyword", "`eps`"),
        "BOOLLIT" => ("a literal", "a boolean literal"),
        "NATLIT" => ("a literal", "a natural number"),
        "HEXLIT" => ("a literal", "a hexadecimal number"),
        "TEXTLIT" => ("a literal", "a text literal"),
        "UPID" => ("an identifier", "an uppercase identifier"),
        "LOID" => ("an identifier", "an identifier"),
        "DOTID" => ("an identifier", "a dot-prefixed identifier"),
        "UPID_LPAREN" => ("an identifier", "an uppercase identifier followed by `(`"),
        "LOID_LPAREN" => ("an identifier", "an identifier followed by `(`"),
        "UPID_LANGLE" => ("an identifier", "an uppercase identifier followed by `<`"),
        "LOID_LANGLE" => ("an identifier", "an identifier followed by `<`"),
        "EOF" => ("end of input", "end of input"),
        _ => ("a token", "a token"),
    }
}
