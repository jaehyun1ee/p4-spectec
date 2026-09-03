use std::{fmt, rc::Rc};

/// A source position
#[derive(Clone, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Position {
    pub file: Rc<str>,
    pub line: i64,
    pub column: i64,
}

impl Position {
    /// Constructs a source position
    pub fn new(file: impl Into<Rc<str>>, line: i64, column: i64) -> Self {
        Self {
            file: file.into(),
            line,
            column,
        }
    }
}

impl fmt::Display for Position {
    fn fmt(&self, fmt: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.line == -1 {
            write!(fmt, "0x{:x}", self.column)
        } else {
            write!(fmt, "{}.{}", self.line, self.column + 1)
        }
    }
}

/// A source span between two positions
#[derive(Clone, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Span {
    pub left: Position,
    pub right: Position,
}

impl Span {
    /// Constructs a span from its endpoints
    pub fn new(left: Position, right: Position) -> Self {
        Self { left, right }
    }

    /// Covers all supplied spans
    pub fn over(regions: &[Self]) -> Self {
        let Some((region_h, regions_t)) = regions.split_first() else {
            return Self::default();
        };

        regions_t
            .iter()
            .fold(region_h.clone(), |region_over, region| {
                Self::new(
                    region_over.left.min(region.left.clone()),
                    region_over.right.max(region.right.clone()),
                )
            })
    }
}

impl fmt::Display for Span {
    fn fmt(&self, fmt: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.left.line == 0 && self.left.column == 0 && self.left == self.right {
            return fmt.write_str(&self.left.file);
        }

        write!(fmt, "{}:{}", self.left.file, self.left)?;
        if self.left != self.right {
            write!(fmt, "-{}", self.right)?;
        }
        Ok(())
    }
}

/// A syntax node paired with semantic and source annotations
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct NotePhrase<T, N = ()> {
    pub node: T,
    pub note: N,
    pub span: Span,
}

/// A syntax node paired with its source span
pub type Phrase<T> = NotePhrase<T>;

impl<T: fmt::Display, N> fmt::Display for NotePhrase<T, N> {
    fn fmt(&self, fmt: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(fmt, "{} at {}", self.node, self.span)
    }
}

impl<T: std::error::Error, N: fmt::Debug> std::error::Error for NotePhrase<T, N> {}

/// Builds a syntax node with an explicit source span
#[macro_export]
macro_rules! phrase {
    (node: $node:expr, span: $span:expr $(,)?) => {
        $crate::lang::common::source::NotePhrase {
            node: $node,
            note: (),
            span: $span,
        }
    };
}

/// Builds a syntax node with semantic and source annotations
#[macro_export]
macro_rules! note_phrase {
    (node: $node:expr, note: $note:expr, span: $span:expr $(,)?) => {
        $crate::lang::common::source::NotePhrase {
            node: $node,
            note: ($note).into(),
            span: $span,
        }
    };
}
