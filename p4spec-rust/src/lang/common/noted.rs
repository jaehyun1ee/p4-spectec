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
    fn collect_free(&self, free: &mut IdSet) {
        self.kind.collect_free(free);
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
