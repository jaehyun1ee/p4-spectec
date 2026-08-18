use std::fmt;

// Positions and regions

#[derive(Clone, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Position {
    pub file: String,
    pub line: i64,
    pub column: i64,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Region {
    pub left: Position,
    pub right: Position,
}

impl Position {
    pub fn new(file: impl Into<String>, line: i64, column: i64) -> Self {
        Self {
            file: file.into(),
            line,
            column,
        }
    }
}

impl Region {
    pub fn new(left: Position, right: Position) -> Self {
        Self { left, right }
    }

    pub fn none() -> Self {
        Self::default()
    }
}

impl Position {
    pub fn for_file(file: impl Into<String>) -> Self {
        Self::new(file, 0, 0)
    }
}

impl Region {
    pub fn for_file(file: impl Into<String>) -> Self {
        let position = Position::for_file(file);
        Self::new(position.clone(), position)
    }

    pub fn before(&self) -> Self {
        Self::new(self.left.clone(), self.left.clone())
    }

    pub fn after(&self) -> Self {
        Self::new(self.right.clone(), self.right.clone())
    }
}

impl fmt::Display for Position {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.line == -1 {
            write!(formatter, "0x{:x}", self.column)
        } else {
            write!(formatter, "{}.{}", self.line, self.column + 1)
        }
    }
}

impl fmt::Display for Region {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self == &Self::for_file(&self.left.file) {
            return formatter.write_str(&self.left.file);
        }
        write!(formatter, "{}:{}", self.left.file, self.left)?;
        if self.left != self.right {
            write!(formatter, "-{}", self.right)?;
        }
        Ok(())
    }
}

// Spans and spanned nodes

pub type Span = Region;

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Spanned<T> {
    pub node: T,
    pub span: Span,
}

pub trait HasSpan {
    fn span(&self) -> &Span;
}

impl<T> Spanned<T> {
    pub fn new(node: T, span: Span) -> Self {
        Self { node, span }
    }

    pub fn map<U>(self, map: impl FnOnce(T) -> U) -> Spanned<U> {
        Spanned {
            node: map(self.node),
            span: self.span,
        }
    }
}

impl<T> HasSpan for Spanned<T> {
    fn span(&self) -> &Span {
        &self.span
    }
}

impl Region {
    pub fn over(regions: &[Self]) -> Self {
        let Some((first, rest)) = regions.split_first() else {
            return Self::none();
        };
        rest.iter().fold(first.clone(), |region_over, region| {
            Self::new(
                region_over.left.min(region.left.clone()),
                region_over.right.max(region.right.clone()),
            )
        })
    }
}

pub fn phrase_list_region<T: HasSpan>(nodes: &[T]) -> Region {
    match nodes {
        [] => Region::none(),
        [node] => node.span().clone(),
        [first, .., last] => Region::new(first.span().left.clone(), last.span().right.clone()),
    }
}
