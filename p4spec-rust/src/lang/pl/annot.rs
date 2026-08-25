use crate::lang::{
    hints::{alter, fields},
    sl,
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

#[cfg(test)]
mod tests {
    use crate::domain::source::{Span, Spanned};

    use super::{Annotated, Hints};

    #[test]
    fn annotated_wrapper_operations_preserve_hints() {
        let annotated = Annotated::new(Spanned::new("source".to_owned(), Span::default()));
        let borrowed = annotated.as_ref();
        assert_eq!(borrowed.node.node, "source");
        assert_eq!(borrowed.hints, Hints::default());

        let (node, hints) = annotated
            .map(|node| Spanned::new(node.node.len(), node.span))
            .into_parts();
        assert_eq!(node.node, 6);
        assert_eq!(node.span, Span::default());
        assert_eq!(hints, Hints::default());
    }
}
