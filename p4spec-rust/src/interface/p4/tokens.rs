//! Adapt located lexer tokens to LALRPOP's location-token-location stream.
//!
//! Each source position is interned in the parser context and recovered later
//! when semantic actions form spans; lexical errors pass through unchanged.

use crate::lang::common::source::Phrase;

use super::{
    context::{Context, Location},
    error::P4Error,
    lexer::Token,
};

pub(crate) fn parser_tokens<'a, I>(
    context: &'a Context,
    tokens: I,
) -> impl Iterator<Item = Result<(Location, Token, Location), P4Error>> + 'a
where
    I: Iterator<Item = Result<Phrase<Token>, P4Error>> + 'a,
{
    tokens.map(|token| {
        token.map(|token| {
            let left = context.location(token.span.left);
            let right = context.location(token.span.right);
            (left, token.node, right)
        })
    })
}
