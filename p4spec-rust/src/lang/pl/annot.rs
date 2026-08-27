//! Prose-language node annotations

use crate::lang::{
    hints::{alter, fields},
    sl,
    traits::eq::SyntaxEq,
};

// Hints

/// Optional prose metadata for a PL node
#[derive(Clone, Debug, PartialEq, Default)]
pub struct Hints {
    pub prose: Option<alter::AlterationHint>,
    pub prose_in: Option<alter::AlterationHint>,
    pub prose_out: Option<alter::AlterationHint>,
    pub prose_true: Option<alter::AlterationHint>,
    pub prose_false: Option<alter::AlterationHint>,
    pub prose_fields: Option<fields::FieldHint>,
    pub prose_input_exps: Option<Vec<sl::ast::Exp>>,
    pub prose_output_exps: Option<Vec<sl::ast::Exp>>,
}

/// A PL node paired with prose metadata
///
/// Does not implement `Deref`;
/// access node and hints explicitly
#[derive(Clone, Debug, PartialEq)]
pub struct Annotated<N> {
    pub node: N,
    pub hints: Hints,
}

impl<N: SyntaxEq> SyntaxEq for Annotated<N> {
    fn syntax_eq(&self, other: &Self) -> bool {
        self.node.syntax_eq(&other.node)
    }
}

impl<N> Annotated<N> {
    /// Builds a node with no prose hints
    pub fn new(node: N) -> Self {
        Self {
            node,
            hints: Hints::default(),
        }
    }

    /// Maps the node;
    /// preserves prose hints
    pub fn map<M>(self, map: impl FnOnce(N) -> M) -> Annotated<M> {
        Annotated {
            node: map(self.node),
            hints: self.hints,
        }
    }

    /// Borrows the node;
    /// clones prose hints
    pub fn as_ref(&self) -> Annotated<&N> {
        Annotated {
            node: &self.node,
            hints: self.hints.clone(),
        }
    }

    /// Splits node ownership from hint ownership
    pub fn into_parts(self) -> (N, Hints) {
        (self.node, self.hints)
    }
}

/// Builds an annotated syntax node with the span of another syntax node
#[macro_export]
macro_rules! annotated {
    (node: $node:expr, span: $span:expr $(,)?) => {
        $crate::lang::pl::annot::Annotated::new($crate::spanned! {
            node: $node,
            span: $span,
        })
    };
}

#[cfg(test)]
mod tests {
    use crate::lang::traits::eq::SyntaxEq;

    use super::Annotated;

    struct SyntaxNode(&'static str);

    impl SyntaxEq for SyntaxNode {
        fn syntax_eq(&self, other: &Self) -> bool {
            self.0 == other.0
        }
    }

    #[test]
    fn annotated_syntax_equality_delegates_to_the_node() {
        let node_first = Annotated::new(SyntaxNode("same"));
        let mut node_second = Annotated::new(SyntaxNode("same"));
        node_second.hints.prose_input_exps = Some(Vec::new());

        assert!(node_first.syntax_eq(&node_second));
    }
}
