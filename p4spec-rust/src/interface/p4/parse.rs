//! Entry points for preprocessing and parsing P4 into runtime values.
//!
//! `parse_file` preprocesses includes, the lexer classifies tokens with a fresh
//! context, and LALRPOP builds the mixfix value tree. For example, parsing an
//! empty source yields the grammar's empty `p4program` case value.

use std::{
    path::{Path, PathBuf},
    rc::Rc,
};

use lalrpop_util::ParseError;

use crate::{
    lang::common::source::{Position, Span},
    lang::data::value::Value,
};

use super::{
    context::{Context, Location},
    error::{P4Error, P4ErrorKind},
    lexer::{Lexer, Token},
    parser::p4programParser,
    preprocessor::preprocess,
    tokens::parser_tokens,
};

// == Entry points

/// Parses an already-preprocessed P4 source string.
pub fn parse_string(path: impl AsRef<Path>, source: &str) -> Result<Rc<Value>, P4Error> {
    let file: Rc<str> = Rc::from(path.as_ref().to_string_lossy().into_owned());
    let context = Rc::new(Context::new());
    let lexer = Lexer::new(Rc::clone(&file), source, Rc::clone(&context));
    let tokens = parser_tokens(context.as_ref(), lexer);

    let result = p4programParser::new().parse(context.as_ref(), tokens);
    result.map_err(|error| syntax_error(context.as_ref(), file, error))
}

/// Preprocesses and parses a P4 source file.
pub fn parse_file(includes: &[PathBuf], path: impl AsRef<Path>) -> Result<Rc<Value>, P4Error> {
    let path = path.as_ref();
    let source = preprocess(includes, path)?;
    parse_string(path, &source)
}

// == Error translation

fn syntax_error(
    context: &Context,
    file: Rc<str>,
    error: ParseError<Location, Token, P4Error>,
) -> P4Error {
    let span = match error {
        ParseError::InvalidToken { location } | ParseError::UnrecognizedEof { location, .. } => {
            point_span(context, file, location)
        }
        ParseError::UnrecognizedToken {
            token: (left, _, right),
            ..
        }
        | ParseError::ExtraToken {
            token: (left, _, right),
        } => context.span(left, right),
        ParseError::User { error } => return error,
    };
    P4Error::new(P4ErrorKind::Syntax, span)
}

fn point_span(context: &Context, file: Rc<str>, location: Location) -> Span {
    let position = context.position(location);
    if position.file.is_empty() {
        let fallback = Position::new(file, position.line, position.column);
        Span::new(fallback.clone(), fallback)
    } else {
        Span::new(position.clone(), position)
    }
}
