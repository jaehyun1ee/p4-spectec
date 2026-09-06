//! String and filesystem entry points for the STF parser
//!
//! `parse_file` reads a source before delegating to `parse_str`. Located lexer
//! tokens become LALRPOP input, statements retain their source spans, and
//! parse failures become typed errors. For example, `wait` becomes one located
//! `Statement::Wait`.

use std::{fs, path::Path, rc::Rc};

use lalrpop_util::ParseError;

use crate::lang::common::source::{Phrase, Position, Span};

use super::{
    ast::Program,
    error::{StfError, StfErrorKind},
    lexer::{Lexer, Token},
    parser,
};

const MAX_PRIORITY: i64 = i64::MAX / 2;

// == Parsing

// - LALRPOP bridge

/// A copyable line and column within the source being parsed.
#[derive(Clone, Copy, Debug, Default)]
pub(crate) struct Location {
    line: i64,
    column: i64,
}

impl Location {
    fn from_position(position: Position) -> Self {
        Self {
            line: position.line,
            column: position.column,
        }
    }

    fn into_position(self, file: Rc<str>) -> Position {
        Position::new(file, self.line, self.column)
    }
}

pub(crate) fn location_span(file: &Rc<str>, location_l: Location, location_r: Location) -> Span {
    let pos_l = location_l.into_position(Rc::clone(file));
    let pos_r = location_r.into_position(Rc::clone(file));
    Span::new(pos_l, pos_r)
}

fn parser_input<I>(tokens: I) -> impl Iterator<Item = Result<(Location, Token, Location), StfError>>
where
    I: Iterator<Item = Result<Phrase<Token>, StfError>>,
{
    tokens.map(|token| {
        token.map(|token| {
            let span = token.span;
            let location_l = Location::from_position(span.left);
            let location_r = Location::from_position(span.right);
            (location_l, token.node, location_r)
        })
    })
}

fn translate_lalrpop_error(
    file: &Rc<str>,
    error: ParseError<Location, Token, StfError>,
) -> StfError {
    let (kind, span) = match error {
        ParseError::InvalidToken { location } => {
            let span = location_span(file, location, location);
            (StfErrorKind::InvalidToken, span)
        }
        ParseError::UnrecognizedEof { location, .. } => {
            let span = location_span(file, location, location);
            (StfErrorKind::UnexpectedEndOfInput, span)
        }
        ParseError::UnrecognizedToken {
            token: (location_l, _, location_r),
            ..
        } => (
            StfErrorKind::UnexpectedToken,
            location_span(file, location_l, location_r),
        ),
        ParseError::ExtraToken {
            token: (location_l, _, location_r),
        } => (
            StfErrorKind::ExtraToken,
            location_span(file, location_l, location_r),
        ),
        ParseError::User { error } => return error,
    };
    StfError::new(kind, span)
}

pub(crate) fn parse_priority(spelling: String, span: Span) -> Result<i64, StfError> {
    let priority = spelling.parse::<i64>().ok();
    match priority.filter(|priority| *priority <= MAX_PRIORITY) {
        Some(priority) => Ok(priority),
        None => {
            let kind = StfErrorKind::InvalidPriority(spelling);
            Err(StfError::new(kind, span))
        }
    }
}

// - Source strings

/// Parses an STF source string and retains statement source spans.
pub fn parse_str(file: impl Into<Rc<str>>, source: &str) -> Result<Program, StfError> {
    let file = file.into();
    let lexer = Lexer::new(Rc::clone(&file), source);
    let input = parser_input(lexer);
    let result = parser::StatementsParser::new().parse(&file, input);
    result.map_err(|error| translate_lalrpop_error(&file, error))
}

// - Source files

/// Reads and parses an STF file.
pub fn parse_file(path: impl AsRef<Path>) -> Result<Program, StfError> {
    let path = path.as_ref();
    let file = Rc::<str>::from(path.to_string_lossy().into_owned());
    let source = fs::read_to_string(path).map_err(|error| {
        let position = Position::new(Rc::clone(&file), 0, 0);
        StfError::new(
            StfErrorKind::Io(error),
            Span::new(position.clone(), position),
        )
    })?;
    parse_str(file, &source)
}
