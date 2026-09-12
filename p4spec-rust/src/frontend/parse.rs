//! String and filesystem entry points for the SpecTec parser

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
    error::{FrontendError, SyntaxErrorKind},
    lexer::{Lexer, Token},
    parser,
    tokens::parser_tokens,
};

/// Parses a SpecTec string with no filesystem source name
pub fn parse_string(source: &str) -> Result<Spec, FrontendError> {
    parse_source(Rc::from(""), source, &Context::default())
}

/// Parses the notation shape syntax used by runtime case constructors.
pub fn parse_mixop(source: &str) -> Result<Mixop, FrontendError> {
    fn from_typ(typ: &ast::Typ) -> Mixop {
        match typ {
            ast::Typ::Plain(_) => Mixfix::Arg(()),
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

    let ctx = Context::default();
    let lexer = Lexer::new(Rc::from(""), source, |id| ctx.find_id(id));
    let tokens = parser_tokens(&ctx, lexer);
    let result = parser::CheckTypParser::new().parse(&ctx, tokens);
    let typ = result.map_err(|error| parse_error(&ctx, error))?;
    let mixop = from_typ(&typ);
    Ok(mixop)
}

fn parse_source(name: Rc<str>, source: &str, ctx: &Context) -> Result<Spec, FrontendError> {
    let lexer = Lexer::new(name, source, |id| ctx.find_id(id));
    let tokens = parser_tokens(ctx, lexer);
    let result = parser::SpecParser::new().parse(ctx, tokens);
    result.map_err(|error| parse_error(ctx, error))
}

fn parse_error(ctx: &Context, error: ParseError<Location, Token, FrontendError>) -> FrontendError {
    let (kind, loc_l, loc_r) = match error {
        ParseError::InvalidToken { location: loc } => (SyntaxErrorKind::InvalidToken, loc, loc),
        ParseError::UnrecognizedEof { location: loc, .. } => {
            (SyntaxErrorKind::UnexpectedEndOfInput, loc, loc)
        }
        ParseError::UnrecognizedToken {
            token: (loc_l, _, loc_r),
            ..
        } => (SyntaxErrorKind::UnexpectedToken, loc_l, loc_r),
        ParseError::ExtraToken {
            token: (loc_l, _, loc_r),
        } => (SyntaxErrorKind::ExtraToken, loc_l, loc_r),
        ParseError::User { error } => return error,
    };
    crate::phrase! {
        node: kind,
        span: ctx.span(loc_l, loc_r),
    }
    .into()
}

/// Reads and parses one SpecTec file
pub fn parse_file(path: impl AsRef<Path>) -> Result<Spec, FrontendError> {
    parse_file_with_context(path.as_ref(), &Context::default())
}

fn parse_file_with_context(path: &Path, ctx: &Context) -> Result<Spec, FrontendError> {
    let name = Rc::<str>::from(path.to_string_lossy().into_owned());
    let position = Position::new(Rc::clone(&name), 0, 0);
    let file_span = Span::new(position.clone(), position);
    let bytes = fs::read(path).map_err(|source| {
        FrontendError::Io(crate::phrase! {
            node: source,
            span: file_span,
        })
    })?;
    let source = str::from_utf8(&bytes).map_err(|source| {
        FrontendError::InvalidUtf8(crate::phrase! {
            node: source,
            span: invalid_utf8_span(Rc::clone(&name), &bytes, &source),
        })
    })?;
    parse_source(name, source, ctx)
}

fn invalid_utf8_span(name: Rc<str>, bytes: &[u8], error: &str::Utf8Error) -> Span {
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
    let pos_l = Position::new(Rc::clone(&name), line, column);
    let pos_r = Position::new(name, line, column + invalid_length);
    Span::new(pos_l, pos_r)
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
        let ctx = Context::with_bindings(Rc::clone(&bindings));
        let defs = parse_file_with_context(&file, &ctx)?;
        spec.extend(defs);
    }
    Ok(spec)
}

fn expand_path(path: &Path, files: &mut Vec<PathBuf>) -> Result<(), FrontendError> {
    let name = path.to_string_lossy().into_owned();
    let position = Position::new(name, 0, 0);
    let span = Span::new(position.clone(), position);
    let metadata = fs::metadata(path).map_err(|source| {
        FrontendError::Io(crate::phrase! {
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
            FrontendError::Io(crate::phrase! {
                node: source,
                span: span.clone(),
            })
        })?
        .collect::<Result<Vec<_>, io::Error>>()
        .map_err(|source| {
            FrontendError::Io(crate::phrase! {
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
            FrontendError::Io(crate::phrase! {
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
