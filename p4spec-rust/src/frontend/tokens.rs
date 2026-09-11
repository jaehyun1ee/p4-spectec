//! Contextual token adaptation between the lexer and LALRPOP
//!
//! `parser_tokens` wraps a lexer iterator in [`ParserTokens`]. Each
//! `ParserTokens::next` call converts source positions to [`Location`]
//! handles and forwards lexical failures as [`FrontendError`]. Outside
//! arithmetic mode it relabels `Star` as `IterStar` for postfix iteration.
//!
//! `ends_sequence` and `starts_sequence` identify adjacent notation atoms. If
//! both predicates match, `ParserTokens::next` returns a synthetic `Sequence`
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

use crate::lang::common::source::{Phrase, Position};

use super::{
    ctx::{Context, Location},
    error::{FrontendError, LexError},
    lexer::Token,
};

pub(crate) fn parser_tokens<I>(ctx: &Context, lexemes: I) -> ParserTokens<'_, I>
where
    I: Iterator,
{
    ParserTokens {
        ctx,
        lexemes,
        previous_right: None,
        previous_token: None,
        pending: None,
    }
}

pub(crate) struct ParserTokens<'ctx, I: Iterator> {
    ctx: &'ctx Context,
    lexemes: I,
    previous_right: Option<Position>,
    previous_token: Option<Token>,
    pending: Option<Phrase<Token>>,
}

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
    I: Iterator<Item = Result<Phrase<Token>, LexError>>,
{
    type Item = Result<(Location, Token, Location), FrontendError>;

    fn next(&mut self) -> Option<Self::Item> {
        let mut lexeme = match self.pending.take() {
            Some(lexeme) => lexeme,
            None => match self.lexemes.next()? {
                Ok(lexeme) => lexeme,
                Err(error) => return Some(Err(error.into())),
            },
        };

        if lexeme.node == Token::Star && !self.ctx.in_arith() {
            lexeme.node = Token::IterStar;
        }

        if self.previous_token.as_ref().is_some_and(ends_sequence) && starts_sequence(&lexeme.node)
        {
            let loc_l = self
                .previous_right
                .clone()
                .expect("previous token position");
            let loc_r = lexeme.span.left.clone();
            self.pending = Some(lexeme);
            self.previous_token = Some(Token::Sequence);
            self.previous_right = Some(loc_r.clone());
            return Some(Ok((
                self.ctx.location(loc_l),
                Token::Sequence,
                self.ctx.location(loc_r),
            )));
        }

        let loc_l = self.ctx.location(lexeme.span.left);
        self.previous_right = Some(lexeme.span.right.clone());
        let loc_r = self.ctx.location(lexeme.span.right);
        self.previous_token = Some(lexeme.node.clone());
        Some(Ok((loc_l, lexeme.node, loc_r)))
    }
}
