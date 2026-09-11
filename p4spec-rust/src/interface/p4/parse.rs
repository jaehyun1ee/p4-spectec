//! Preprocessing and parsing P4 into runtime values
//!
//! `parse_file` preprocesses includes before delegating to `parse_string`, which
//! creates a fresh name-resolution context, lexes the source, adapts located
//! tokens for LALRPOP, and builds the mixfix value tree. Parse failures retain
//! lexer locations through the same context. For example, an empty source
//! produces the grammar's empty `p4program` case value.

use std::{
    path::{Path, PathBuf},
    rc::Rc,
};

use lalrpop_util::ParseError;

use crate::{
    lang::common::source::{Phrase, Position, Span},
    lang::data::value::{Value, ValueArena},
};

use super::{
    context::{Context, Location},
    error::{P4Error, P4ErrorKind},
    lexer::{Lexer, Token},
    parser::p4programParser,
    preprocessor::preprocess,
};

// == Parsing

// - LALRPOP bridge

fn parser_input<'a, I>(
    context: &'a Context,
    tokens: I,
    position: Position,
) -> impl Iterator<Item = Result<(Location, Token, Location), P4Error>> + 'a
where
    I: Iterator<Item = Result<Phrase<Token>, P4Error>> + 'a,
{
    let mut location_prev = context.location_add(position, None);
    tokens.map(move |token| {
        token.map(|token| {
            let location_l = context.location_add(token.span.left, Some(location_prev));
            let location_r = context.location_add(token.span.right, None);
            location_prev = location_r;
            (location_l, token.node, location_r)
        })
    })
}

fn translate_lalrpop_error(
    context: &Context,
    error: ParseError<Location, Token, P4Error>,
) -> P4Error {
    let span = match error {
        ParseError::InvalidToken { location } | ParseError::UnrecognizedEof { location, .. } => {
            let position = context.location_get(location);
            Span::new(position.clone(), position)
        }
        ParseError::UnrecognizedToken {
            token: (location_l, _, location_r),
            ..
        }
        | ParseError::ExtraToken {
            token: (location_l, _, location_r),
        } => context.location_span(location_l, location_r),
        ParseError::User { error } => return error,
    };
    P4Error::new(P4ErrorKind::Syntax, span)
}

// - Source strings

/// Parses an already-preprocessed P4 source string.
pub fn parse_string(
    arena: &mut ValueArena,
    path: impl AsRef<Path>,
    source: &str,
) -> Result<Value, P4Error> {
    let file: Rc<str> = Rc::from(path.as_ref().to_string_lossy().into_owned());
    let context = Rc::new(Context::new(arena));
    let position = Position::new(Rc::clone(&file), 1, 0);
    let lexer = Lexer::new(file, source, Rc::clone(&context));
    let input = parser_input(context.as_ref(), lexer, position);

    let result = p4programParser::new().parse(context.as_ref(), input);
    result.map_err(|error| translate_lalrpop_error(context.as_ref(), error))
}

// - Source files

/// Preprocesses and parses a P4 source file.
pub fn parse_file(
    arena: &mut ValueArena,
    includes: &[PathBuf],
    path: impl AsRef<Path>,
) -> Result<Value, P4Error> {
    let path = path.as_ref();
    let source = preprocess(includes, path)?;
    parse_string(arena, path, &source)
}
