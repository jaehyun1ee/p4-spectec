//! String and filesystem entry points for the STF parser
//!
//! `parse_file` reads a source before delegating to `parse_str`. A source map
//! converts lexer byte offsets to positions, LALRPOP builds statements in
//! order, and parse failures become typed errors with source spans. For
//! example, `wait` becomes one located `Statement::Wait`.

use std::{fs, path::Path, rc::Rc};

use lalrpop_util::ParseError;

use crate::{
    lang::common::source::{Phrase, Position, Span},
    phrase,
};

use super::{
    ast::{Program, Statement},
    error::{StfError, StfErrorKind},
    lexer::{Lexer, Token},
    parser,
};

const MAX_PRIORITY: i64 = i64::MAX / 2;

// == Source locations

pub(crate) struct SourceMap {
    file: Rc<str>,
    source: Rc<str>,
}

impl SourceMap {
    // - Construction

    fn new(file: Rc<str>, source: &str) -> Self {
        Self {
            file,
            source: source.into(),
        }
    }

    // - Positions and spans

    pub(crate) fn position(&self, offset: usize) -> Position {
        let prefix = &self.source[..offset.min(self.source.len())];
        let line = prefix.bytes().filter(|byte| *byte == b'\n').count() as i64 + 1;
        let line_start = prefix.rfind('\n').map_or(0, |index| index + 1);
        Position::new(Rc::clone(&self.file), line, (offset - line_start) as i64)
    }

    pub(crate) fn span(&self, left: usize, right: usize) -> Span {
        Span::new(self.position(left), self.position(right))
    }

    pub(crate) fn phrase(&self, node: Statement, left: usize, right: usize) -> Phrase<Statement> {
        let span = self.span(left, right);
        phrase! { node: node, span: span }
    }

    // - Numeric fields

    pub(crate) fn priority(
        &self,
        value: String,
        left: usize,
        right: usize,
    ) -> Result<i64, StfError> {
        let priority = value.parse::<i64>().ok();
        match priority.filter(|priority| *priority <= MAX_PRIORITY) {
            Some(priority) => Ok(priority),
            None => {
                let kind = StfErrorKind::InvalidPriority(value);
                let span = self.span(left, right);
                Err(StfError::new(kind, span))
            }
        }
    }
}

// == Parsing

// - Source strings

/// Parses an STF source string and retains statement source spans.
pub fn parse_str(file: impl Into<Rc<str>>, source: &str) -> Result<Program, StfError> {
    let source_map = SourceMap::new(file.into(), source);
    let lexer = Lexer::new(Rc::clone(&source_map.file), source);
    let result = parser::StatementsParser::new().parse(&source_map, lexer);
    result.map_err(|error| translate_parse_error(&source_map, error))
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

// == Error translation

fn translate_parse_error(
    source_map: &SourceMap,
    error: ParseError<usize, Token, StfError>,
) -> StfError {
    let (kind, left, right) = match error {
        ParseError::InvalidToken { location } => (StfErrorKind::InvalidToken, location, location),
        ParseError::UnrecognizedEof { location, .. } => {
            (StfErrorKind::UnexpectedEndOfInput, location, location)
        }
        ParseError::UnrecognizedToken {
            token: (left, _, right),
            ..
        } => (StfErrorKind::UnexpectedToken, left, right),
        ParseError::ExtraToken {
            token: (left, _, right),
        } => (StfErrorKind::ExtraToken, left, right),
        ParseError::User { error } => return error,
    };
    let span = source_map.span(left, right);
    StfError::new(kind, span)
}
