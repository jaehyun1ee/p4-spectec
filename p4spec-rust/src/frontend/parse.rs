//! Filesystem and notation-shape entry points for the SpecTec parser
//!
//! `parse_files` reads `.watsup` files in order,
//! sharing variable bindings across them,
//! and returns their definitions as one `Spec`.
//! `parse_mixop` parses a notation type and keeps only its shape,
//! for the case constructors that name their mixop as text.

use std::{
    fs, io,
    path::{Path, PathBuf},
    rc::Rc,
    str,
};

use lalrpop_util::ParseError;

use crate::lang::{
    common::{
        notation::{mixfix::Mixfix, mixop::Mixop},
        source::{Position, Span},
    },
    el::ast::{self, Spec},
};

use super::{
    ctx::{Bindings, Context, Location},
    error::{self, FrontendError},
    lexer::{Lexer, Token},
    parser,
    tokens::parser_tokens,
};

/// Parses the notation shape syntax used by runtime case constructors.
pub fn parse_mixop(source: &str) -> Result<Mixop, FrontendError> {
    /// Replaces every type in a notation type with an argument hole.
    fn from_typ(typ: &ast::Typ) -> Mixop {
        match typ {
            // A plain type is an argument position
            ast::Typ::Plain(_) => Mixfix::Arg(()),
            // Notation keeps its atoms and recurses into its parts
            ast::Typ::Notation(notation) => match &notation.node {
                ast::NotTypKind::Atom(atom) => {
                    let atom = atom.clone();
                    Mixfix::Atom(atom)
                }
                ast::NotTypKind::Seq(types) => {
                    let mixfixes = types.iter().map(from_typ).collect();
                    Mixfix::Seq(mixfixes)
                }
                ast::NotTypKind::Infix(typ_l, atom, typ_r) => {
                    let typ_l = Box::new(from_typ(typ_l));
                    let atom = atom.clone();
                    let typ_r = Box::new(from_typ(typ_r));
                    Mixfix::Infix(typ_l, atom, typ_r)
                }
                ast::NotTypKind::Brack(atom_l, typ_inner, atom_r) => {
                    let atom_l = atom_l.clone();
                    let typ_inner = Box::new(from_typ(typ_inner));
                    let atom_r = atom_r.clone();
                    Mixfix::Brack(atom_l, typ_inner, atom_r)
                }
            },
        }
    }

    // Parse as a type with a throwaway context
    let ctx = Context::default();
    let lexer = Lexer::new(Rc::from(""), source, |id| ctx.find_id(id));
    let tokens = parser_tokens(&ctx, lexer);
    let result = parser::CheckTypParser::new().parse(&ctx, tokens);
    let typ = result.map_err(|error_parse| match error_parse {
        ParseError::User { error } => error,
        _ => error::mixfix_operator_invalid(source),
    })?;
    let mixop = from_typ(&typ);
    Ok(mixop)
}

/// Lexes and parses one source text into definitions.
fn parse_source_with_context(
    name: Rc<str>,
    source: &str,
    ctx: &Context,
) -> Result<Spec, FrontendError> {
    let lexer = Lexer::new(name, source, |id| ctx.find_id(id));
    let tokens = parser_tokens(ctx, lexer);
    let result = parser::SpecParser::new().parse(ctx, tokens);
    result.map_err(|error| parse_error(ctx, source, error))
}

/// Maps LALRPOP control failures to reports at the responsible token.
fn parse_error(
    ctx: &Context,
    source: &str,
    error_parse: ParseError<Location, Token, FrontendError>,
) -> FrontendError {
    match error_parse {
        ParseError::InvalidToken { location: loc } => {
            error::token_invalid(ctx.span(loc, loc), None, &[])
        }
        ParseError::UnrecognizedEof { location: loc, expected } => {
            error::input_incomplete(ctx.span(loc, loc), &expected)
        }
        ParseError::UnrecognizedToken { token: (loc_l, Token::Eof, loc_r), expected } => {
            error::input_incomplete(ctx.span(loc_l, loc_r), &expected)
        }
        ParseError::UnrecognizedToken { token: (loc_l, token, loc_r), expected } => {
            let span = ctx.span(loc_l, loc_r);
            let actual = describe_token(source, &span, &token);
            error::token_invalid(span, Some(&actual), &expected)
        }
        ParseError::ExtraToken { token: (loc_l, token, loc_r) } => {
            let span = ctx.span(loc_l, loc_r);
            let actual = describe_token(source, &span, &token);
            error::token_invalid(span, Some(&actual), &["EOF".to_owned()])
        }
        ParseError::User { error } => error,
    }
}

/// Uses the lexeme's source spelling, including payloads and punctuation.
fn describe_token(source: &str, span: &Span, token: &Token) -> String {
    // Layout and parser-inserted tokens can have zero-width source spans
    match token {
        Token::Sequence => return "adjacent token".to_owned(),
        Token::Eof => return "end of input".to_owned(),
        Token::NewlineBar => return "newline followed by `|`".to_owned(),
        Token::Newline2 => return "blank line".to_owned(),
        Token::Newline3 => return "two blank lines".to_owned(),
        _ => {}
    }

    // Positions retain byte columns, so source spelling needs no decoding
    let offset = |pos: &Position| {
        source
            .split_inclusive('\n')
            .take(pos.line.saturating_sub(1))
            .map(str::len)
            .sum::<usize>()
            + pos.column
    };
    let text = &source[offset(&span.left)..offset(&span.right)];
    format!("token {text:?}")
}

/// Parses a UTF-8 source string with fresh variable bindings.
pub fn parse_source(name: Rc<str>, source: &str) -> Result<Spec, FrontendError> {
    parse_source_with_context(name, source, &Context::default())
}

/// Validates source bytes and parses them with fresh variable bindings.
pub fn parse_bytes(name: Rc<str>, bytes: &[u8]) -> Result<Spec, FrontendError> {
    parse_bytes_with_context(name, bytes, &Context::default())
}

/// Validates encoding before constructing the UTF-8 lexer.
fn parse_bytes_with_context(
    name: Rc<str>,
    bytes: &[u8],
    ctx: &Context,
) -> Result<Spec, FrontendError> {
    let source = str::from_utf8(bytes).map_err(|error_utf8| {
        let span = invalid_utf8_span(Rc::clone(&name), bytes, &error_utf8);
        // Utf8Error guarantees that the prefix before valid_up_to is valid
        let prefix = str::from_utf8(&bytes[..error_utf8.valid_up_to()])
            .expect("UTF-8 decoder validated the prefix");
        // Classify comment encoding only when lexing reaches the invalid bytes
        let mut lexer = Lexer::new(Rc::clone(&name), prefix, |_| false);
        while lexer.next().is_some_and(|token| token.is_ok()) {}
        if lexer.in_block_comment() {
            error::comment_encoding_invalid(span, bytes, &error_utf8)
        } else {
            // Earlier lexical errors leave the encoding context unknown
            error::source_encoding_invalid(span, bytes, &error_utf8)
        }
    })?;
    parse_source_with_context(name, source, ctx)
}

/// Reads a file and parses its bytes with shared variable bindings.
fn parse_file_with_context(path: &Path, ctx: &Context) -> Result<Spec, FrontendError> {
    let name = Rc::<str>::from(path.to_string_lossy().into_owned());
    let pos = Position::new(Rc::clone(&name), 0, 0);
    let span = Span::new(pos.clone(), pos);
    let bytes = fs::read(path).map_err(|error_io| error::file_read_failed(span, &error_io))?;
    parse_bytes_with_context(name, &bytes, ctx)
}

/// Locates a UTF-8 error by counting newlines in the valid prefix.
fn invalid_utf8_span(name: Rc<str>, bytes: &[u8], error: &str::Utf8Error) -> Span {
    let offset = error.valid_up_to();
    let valid_prefix = &bytes[..offset];
    // Line and column of the first invalid byte
    let line = valid_prefix.iter().filter(|byte| **byte == b'\n').count() + 1;
    let line_start = valid_prefix
        .iter()
        .rposition(|byte| *byte == b'\n')
        .map_or(0, |newline| newline + 1);
    let column = offset - line_start;
    // The span covers the invalid sequence, or the rest if it is truncated
    let invalid_length = error
        .error_len()
        .unwrap_or_else(|| bytes.len().saturating_sub(offset));
    let pos_l = Position::new(Rc::clone(&name), line, column);
    let pos_r = Position::new(name, line, column + invalid_length);
    Span::new(pos_l, pos_r)
}

/// Parses files and directories in order,
/// recursively expanding `.watsup` files in directories
/// while excluding nested `include` directories.
pub fn parse_files<I, P>(paths: I) -> Result<Spec, FrontendError>
where
    I: IntoIterator<Item = P>,
    P: AsRef<Path>,
{
    // Collect every file first so a missing path fails before parsing
    let mut files = Vec::new();
    for path in paths {
        expand_path(path.as_ref(), &mut files)?;
    }

    // Variables bound in one file stay bound in the next
    let bindings = Rc::new(Bindings::default());
    let mut spec = Vec::new();
    for file in files {
        let ctx = Context::with_bindings(Rc::clone(&bindings));
        let defs = parse_file_with_context(&file, &ctx)?;
        spec.extend(defs);
    }
    Ok(spec)
}

/// Adds a file, or the `.watsup` files under a directory, in name order.
fn expand_path(path: &Path, files: &mut Vec<PathBuf>) -> Result<(), FrontendError> {
    // Resolve path failures before parsing any collected files
    let metadata = match fs::metadata(path) {
        Ok(metadata) => metadata,
        Err(error_io) => {
            let pos = Position::new(path.to_string_lossy().into_owned(), 0, 0);
            return Err(error::file_read_failed(Span::new(pos.clone(), pos), &error_io));
        }
    };
    // A file is taken as given, whatever its extension
    if !metadata.is_dir() {
        files.push(path.to_owned());
        return Ok(());
    }

    let mut entries = fs::read_dir(path)
        .map_err(|error_io| error::input_path_read_failed(path, &error_io))?
        .collect::<Result<Vec<_>, io::Error>>()
        .map_err(|error_io| error::input_path_read_failed(path, &error_io))?;
    // Directory order is not stable, so sort by name
    entries.sort_by_key(fs::DirEntry::file_name);

    for entry in entries {
        let entry_path = entry.path();
        let entry_name = entry_path.to_string_lossy().into_owned();
        let entry_metadata = fs::metadata(&entry_path)
            .map_err(|error_io| error::input_path_read_failed(&entry_path, &error_io))?;
        // Recurse, except into `include` directories
        if entry_metadata.is_dir() {
            if entry.file_name() != "include" {
                expand_path(&entry_path, files)?;
            }
        // Only specification files are taken from directories
        } else if entry_name.ends_with(".watsup") {
            files.push(entry_path);
        }
    }
    Ok(())
}
