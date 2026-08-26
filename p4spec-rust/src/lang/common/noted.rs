/// A syntax node paired with a semantic note
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Noted<T, S> {
    pub kind: T,
    pub note: S,
}

impl<T, S> Noted<T, S> {
    /// Builds a node with its semantic note
    pub fn new(kind: T, note: S) -> Self {
        Self { kind, note }
    }
}

/// Builds a noted syntax node with the span of another syntax node
#[macro_export]
macro_rules! noted {
    (kind: $kind:expr, note: $note:expr, span: $span:expr $(,)?) => {
        $crate::spanned! {
            node: $crate::lang::common::noted::Noted::new($kind, $note),
            span: $span,
        }
    };
}

#[cfg(test)]
mod tests {
    use super::super::source::{Position, Span, Spanned};

    #[test]
    fn noted_macro_builds_a_noted_node_with_the_supplied_nodes_span() {
        let source = Spanned::new(
            "source",
            Span::new(Position::new("test", 1, 2), Position::new("test", 3, 4)),
        );

        let result = crate::noted! {
            kind: "kind",
            note: "note",
            span: source,
        };

        assert_eq!(result.node.kind, "kind");
        assert_eq!(result.node.note, "note");
        assert_eq!(result.span, source.span);
    }
}
