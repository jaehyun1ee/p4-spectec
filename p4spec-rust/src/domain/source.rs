use std::fmt;

#[derive(Clone, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Position {
    pub file: String,
    pub line: i64,
    pub column: i64,
}

impl Position {
    pub fn new(file: impl Into<String>, line: i64, column: i64) -> Self {
        Self {
            file: file.into(),
            line,
            column,
        }
    }

    pub fn for_file(file: impl Into<String>) -> Self {
        Self::new(file, 0, 0)
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

#[derive(Clone, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Region {
    pub left: Position,
    pub right: Position,
}

impl Region {
    pub fn new(left: Position, right: Position) -> Self {
        Self { left, right }
    }

    pub fn none() -> Self {
        Self::default()
    }

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

pub type Span = Region;

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Spanned<T> {
    pub node: T,
    pub span: Span,
}

impl<T> Spanned<T> {
    pub fn new(node: T, span: Span) -> Self {
        Self { node, span }
    }
}

pub trait HasSpan {
    fn span(&self) -> &Span;
}

impl<T> HasSpan for Spanned<T> {
    fn span(&self) -> &Span {
        &self.span
    }
}

pub fn phrase_list_region<T: HasSpan>(phrases: &[T]) -> Region {
    match phrases {
        [] => Region::none(),
        [phrase] => phrase.span().clone(),
        [first, .., last] => Region::new(first.span().left.clone(), last.span().right.clone()),
    }
}

#[cfg(test)]
mod tests {
    use super::{Position, Region, Spanned, phrase_list_region};

    fn position(line: i64, column: i64) -> Position {
        Position::new("spec.watsup", line, column)
    }

    #[test]
    fn over_region_spans_outermost_positions() {
        let regions = [
            Region::new(position(4, 2), position(4, 8)),
            Region::new(position(2, 5), position(3, 1)),
            Region::new(position(5, 0), position(7, 3)),
        ];

        assert_eq!(
            Region::over(&regions),
            Region::new(position(2, 5), position(7, 3))
        );
    }

    #[test]
    fn phrase_list_region_uses_first_and_last_phrase_boundaries() {
        let phrases = [
            Spanned::new("first", Region::new(position(4, 2), position(4, 8))),
            Spanned::new("middle", Region::new(position(1, 0), position(9, 0))),
            Spanned::new("last", Region::new(position(5, 0), position(7, 3))),
        ];

        assert_eq!(
            phrase_list_region(&phrases),
            Region::new(position(4, 2), position(7, 3))
        );
    }
}
