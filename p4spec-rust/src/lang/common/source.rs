use std::fmt;

/// A source position
#[derive(Clone, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Position {
    pub file: String,
    pub line: i64,
    pub column: i64,
}

impl Position {
    /// Constructs a source position
    pub fn new(file: impl Into<String>, line: i64, column: i64) -> Self {
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

/// A syntax node paired with its source span
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Spanned<T> {
    pub node: T,
    pub span: Span,
}

impl<T> Spanned<T> {
    /// Builds a node with an explicit source span
    pub fn new(node: T, span: Span) -> Self {
        Self { node, span }
    }
}

/// Builds a syntax node with the span of another syntax node
#[macro_export]
macro_rules! spanned {
    (node: $node:expr, span: $span:expr $(,)?) => {{
        let span = $span.span.clone();
        $crate::lang::common::source::Spanned::new($node, span)
    }};
}

#[cfg(test)]
mod tests {
    use super::{Position, Span, Spanned};

    #[test]
    fn spanned_macro_copies_the_supplied_nodes_span() {
        let source = Spanned::new(
            "source",
            Span::new(Position::new("test", 1, 2), Position::new("test", 3, 4)),
        );

        let result = crate::spanned! {
            node: "result",
            span: source,
        };

        assert_eq!(result.node, "result");
        assert_eq!(result.span, source.span);
    }
}
