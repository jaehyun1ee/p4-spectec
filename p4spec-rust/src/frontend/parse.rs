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
    ctx::{Bindings, Context, Location},
    error::{FrontendError, SyntaxErrorKind},
    lexer::{Lexer, Token},
    parser,
    tokens::parser_tokens,
};

/// Parses a SpecTec string with no filesystem source name
pub fn parse_string(source: &str) -> Result<Spec, FrontendError> {
    parse_source("", source, &Context::default())
}

fn parse_source(name: &str, source: &str, context: &Context) -> Result<Spec, FrontendError> {
    let lexer = Lexer::new(name, source, |id| context.find_id(id));
    parser::SpecParser::new()
        .parse(context, parser_tokens(context, lexer))
        .map_err(|error| parse_error(context, error))
}

fn parse_error(
    context: &Context,
    error: ParseError<Location, Token, FrontendError>,
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
    crate::spanned! {
        node: kind,
        span: context.span(left, right),
    }
    .into()
}

/// Reads and parses one SpecTec file
pub fn parse_file(path: impl AsRef<Path>) -> Result<Spec, FrontendError> {
    parse_file_with_context(path.as_ref(), &Context::default())
}

fn parse_file_with_context(path: &Path, context: &Context) -> Result<Spec, FrontendError> {
    let name = path.to_string_lossy().into_owned();
    let position = Position::new(name.clone(), 0, 0);
    let file_span = Span::new(position.clone(), position);
    let bytes = fs::read(path).map_err(|source| {
        FrontendError::Io(crate::spanned! {
            node: source,
            span: file_span,
        })
    })?;
    let source = str::from_utf8(&bytes).map_err(|source| {
        FrontendError::InvalidUtf8(crate::spanned! {
            node: source,
            span: invalid_utf8_span(&name, &bytes, &source),
        })
    })?;
    parse_source(&name, source, context)
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

    let bindings = Rc::new(Bindings::default());
    let mut spec = Vec::new();
    for file in files {
        let context = Context::with_bindings(Rc::clone(&bindings));
        spec.extend(parse_file_with_context(&file, &context)?);
    }
    Ok(spec)
}

fn expand_path(path: &Path, files: &mut Vec<PathBuf>) -> Result<(), FrontendError> {
    let name = path.to_string_lossy().into_owned();
    let position = Position::new(name, 0, 0);
    let span = Span::new(position.clone(), position);
    let metadata = fs::metadata(path).map_err(|source| {
        FrontendError::Io(crate::spanned! {
            node: source,
            span: span.clone(),
        })
    })?;
    if !metadata.is_dir() {
        files.push(path.to_owned());
        return Ok(());
    }

    let mut entries = fs::read_dir(path)
        .map_err(|source| {
            FrontendError::Io(crate::spanned! {
                node: source,
                span: span.clone(),
            })
        })?
        .collect::<Result<Vec<_>, io::Error>>()
        .map_err(|source| {
            FrontendError::Io(crate::spanned! {
                node: source,
                span: span,
            })
        })?;
    entries.sort_by_key(fs::DirEntry::file_name);

    for entry in entries {
        let entry_path = entry.path();
        let entry_name = entry_path.to_string_lossy().into_owned();
        let position = Position::new(entry_name.clone(), 0, 0);
        let span = Span::new(position.clone(), position);
        let entry_metadata = fs::metadata(&entry_path).map_err(|source| {
            FrontendError::Io(crate::spanned! {
                node: source,
                span: span,
            })
        })?;
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
