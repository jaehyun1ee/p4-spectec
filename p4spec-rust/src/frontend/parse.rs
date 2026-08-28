//! String and filesystem entry points for the SpecTec parser

use std::{
    fs, io,
    path::{Path, PathBuf},
    rc::Rc,
    str,
};

use lalrpop_util::ParseError;

use crate::lang::{
    common::source::{Position, Span},
    el::ast::Spec,
};

use super::{
    error::{FrontendError, SyntaxError, SyntaxErrorKind},
    lexer::{Lexer, Token},
    parser,
    parser_support::{ParserBindings, ParserContext, ParserLocation, parser_tokens},
};

/// Parses a SpecTec string with no filesystem source name
pub fn parse_string(source: &str) -> Result<Spec, FrontendError> {
    parse_source("", source, &ParserContext::default())
}

/// Reads and parses one SpecTec file
pub fn parse_file(path: impl AsRef<Path>) -> Result<Spec, FrontendError> {
    parse_file_with_context(path.as_ref(), &ParserContext::default())
}

/// Parses files and directories in order, recursively expanding `.watsup` files
/// in directories while excluding nested `include` directories
pub fn parse_files<I, P>(paths: I) -> Result<Spec, FrontendError>
where
    I: IntoIterator<Item = P>,
    P: AsRef<Path>,
{
    let mut files = Vec::new();
    for path in paths {
        expand_path(path.as_ref(), &mut files)?;
    }

    let bindings = Rc::new(ParserBindings::default());
    let mut spec = Vec::new();
    for file in files {
        let context = ParserContext::with_bindings(Rc::clone(&bindings));
        spec.extend(parse_file_with_context(&file, &context)?);
    }
    Ok(spec)
}

fn parse_file_with_context(path: &Path, context: &ParserContext) -> Result<Spec, FrontendError> {
    let name = path.to_string_lossy().into_owned();
    let bytes = fs::read(path).map_err(|error| FrontendError::io(name.clone(), error))?;
    let source = str::from_utf8(&bytes).map_err(|error| {
        FrontendError::invalid_utf8(invalid_utf8_span(&name, &bytes, &error), error)
    })?;
    parse_source(&name, source, context)
}

fn parse_source(name: &str, source: &str, context: &ParserContext) -> Result<Spec, FrontendError> {
    let lexer = Lexer::with_uppercase_classifier(name, source, |id| context.is_var(id));
    parser::SpecParser::new()
        .parse(context, parser_tokens(context, lexer))
        .map_err(|error| parse_error(context, error))
}

fn parse_error(
    context: &ParserContext,
    error: ParseError<ParserLocation, Token, FrontendError>,
) -> FrontendError {
    let (kind, left, right) = match error {
        ParseError::InvalidToken { location } => {
            (SyntaxErrorKind::InvalidToken, location, location)
        }
        ParseError::UnrecognizedEof { location, .. } => {
            (SyntaxErrorKind::UnexpectedEndOfInput, location, location)
        }
        ParseError::UnrecognizedToken {
            token: (left, _, right),
            ..
        } => (SyntaxErrorKind::UnexpectedToken, left, right),
        ParseError::ExtraToken {
            token: (left, _, right),
        } => (SyntaxErrorKind::ExtraToken, left, right),
        ParseError::User { error } => return error,
    };
    SyntaxError::new(kind, context.span(left, right)).into()
}

fn invalid_utf8_span(name: &str, bytes: &[u8], error: &str::Utf8Error) -> Span {
    let offset = error.valid_up_to();
    let valid_prefix = &bytes[..offset];
    let line = valid_prefix.iter().filter(|byte| **byte == b'\n').count() as i64 + 1;
    let line_start = valid_prefix
        .iter()
        .rposition(|byte| *byte == b'\n')
        .map_or(0, |newline| newline + 1);
    let column = (offset - line_start) as i64;
    let invalid_length = error
        .error_len()
        .unwrap_or_else(|| bytes.len().saturating_sub(offset)) as i64;
    let left = Position::new(name, line, column);
    let right = Position::new(name, line, column + invalid_length);
    Span::new(left, right)
}

fn expand_path(path: &Path, files: &mut Vec<PathBuf>) -> Result<(), FrontendError> {
    let name = path.to_string_lossy().into_owned();
    let metadata = fs::metadata(path).map_err(|error| FrontendError::io(name.clone(), error))?;
    if !metadata.is_dir() {
        files.push(path.to_owned());
        return Ok(());
    }

    let mut entries = fs::read_dir(path)
        .map_err(|error| FrontendError::io(name.clone(), error))?
        .collect::<Result<Vec<_>, io::Error>>()
        .map_err(|error| FrontendError::io(name, error))?;
    entries.sort_by_key(fs::DirEntry::file_name);

    for entry in entries {
        let entry_path = entry.path();
        let entry_name = entry_path.to_string_lossy().into_owned();
        let entry_metadata = fs::metadata(&entry_path)
            .map_err(|error| FrontendError::io(entry_name.clone(), error))?;
        if entry_metadata.is_dir() {
            if entry.file_name() != "include" {
                expand_path(&entry_path, files)?;
            }
        } else if entry_name.ends_with(".watsup") {
            files.push(entry_path);
        }
    }
    Ok(())
}
