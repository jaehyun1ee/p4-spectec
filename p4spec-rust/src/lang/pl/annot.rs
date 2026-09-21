//! Prose-language node annotations

use crate::lang::{
    common::ds::set::IdSet,
    hints::{alter, fields},
    sl,
    traits::{eq::SyntaxEq, free::Free},
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

impl<N: Free> Free for Annotated<N> {
    fn free(&self) -> IdSet {
        self.node.free()
    }
}

impl<N> Annotated<N> {
    /// Builds a node with no prose hints
    pub fn new(node: N) -> Self {
        Self { node, hints: Hints::default() }
    }
}

/// Builds a syntax node paired with prose metadata
#[macro_export]
macro_rules! annotated {
    (node: $node:expr, hints: $hints:expr $(,)?) => {
        $crate::lang::pl::annot::Annotated { node: $node, hints: $hints }
    };
    (node: $node:expr, span: $span:expr $(,)?) => {
        $crate::annotated! {
            node: $crate::phrase! {
                node: $node,
                span: $span.span.clone(),
            },
            hints: $crate::lang::pl::annot::Hints::default(),
        }
    };
}

/// Builds a source-annotated syntax node paired with prose metadata
#[macro_export]
macro_rules! annotated_note_phrase {
    (
        node: $node:expr,
        note: $note:expr,
        span: $span:expr,
        hints: $hints:expr $(,)?
    ) => {
        $crate::annotated! {
            node: $crate::note_phrase! {
                node: $node,
                note: $note,
                span: $span,
            },
            hints: $hints,
        }
    };
    (
        node: $node:expr,
        note: $note:expr,
        span: $span:expr $(,)?
    ) => {
        $crate::annotated_note_phrase! {
            node: $node,
            note: $note,
            span: $span,
            hints: $crate::lang::pl::annot::Hints::default(),
        }
    };
}
