use crate::lang::{
    common::ds::set::IdSet,
    traits::{eq::SyntaxEq, free::Free},
};

/// A syntax node paired with a semantic note
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Noted<T, S> {
    pub kind: T,
    pub note: S,
}

impl<T: SyntaxEq, S> SyntaxEq for Noted<T, S> {
    fn syntax_eq(&self, other: &Self) -> bool {
        self.kind.syntax_eq(&other.kind)
    }
}

impl<T: Free, S> Free for Noted<T, S> {
    fn free(&self) -> IdSet {
        self.kind.free()
    }
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
    use crate::lang::{
        common::Id,
        traits::{eq::SyntaxEq, free::Free},
    };

    use super::super::source::{Position, Span, Spanned};
    use super::{IdSet, Noted};

    struct SyntaxNode(&'static str);

    struct FreeNode(Id);

    impl SyntaxEq for SyntaxNode {
        fn syntax_eq(&self, other: &Self) -> bool {
            self.0 == other.0
        }
    }

    impl Free for FreeNode {
        fn free(&self) -> IdSet {
            IdSet::from([self.0.clone()])
        }
    }

    #[test]
    fn noted_syntax_equality_delegates_to_the_kind() {
        let node_first = Noted::new(SyntaxNode("same"), "first note");
        let node_second = Noted::new(SyntaxNode("same"), "second note");

        assert!(node_first.syntax_eq(&node_second));
    }

    #[test]
    fn noted_free_delegates_to_the_kind() {
        let id = Spanned::new("x".to_owned(), Span::default());
        let node = Noted::new(FreeNode(id.clone()), "note");

        assert!(node.free().contains(&id));
    }

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
