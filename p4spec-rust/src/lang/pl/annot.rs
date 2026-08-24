use crate::{
    domain::source::{HasSpan, Span},
    lang::{
        hints::{alter, fields},
        sl,
    },
};

// Hints

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

// Wrap a node with no prose hints

#[derive(Clone, Debug, PartialEq)]
pub struct Annotated<N> {
    pub node: N,
    pub hints: Hints,
}

impl<N> Annotated<N> {
    pub fn new(node: N) -> Self {
        Self {
            node,
            hints: Hints::default(),
        }
    }

    pub fn map<M>(self, map: impl FnOnce(N) -> M) -> Annotated<M> {
        Annotated {
            node: map(self.node),
            hints: self.hints,
        }
    }

    pub fn as_ref(&self) -> Annotated<&N> {
        Annotated {
            node: &self.node,
            hints: self.hints.clone(),
        }
    }

    pub fn into_parts(self) -> (N, Hints) {
        (self.node, self.hints)
    }
}

impl<N: HasSpan> HasSpan for Annotated<N> {
    fn span(&self) -> &Span {
        self.node.span()
    }
}

#[cfg(test)]
mod tests {
    use crate::domain::source::{Region, Spanned};

    use super::{Annotated, Hints};

    #[test]
    fn annotated_wrapper_operations_preserve_hints() {
        let annotated = Annotated::new(Spanned::new(
            "source".to_owned(),
            Region::for_file("spec.watsup"),
        ));
        let borrowed = annotated.as_ref();
        assert_eq!(borrowed.node.node, "source");
        assert_eq!(borrowed.hints, Hints::default());

        let (node, hints) = annotated
            .map(|node| node.map(|node| node.len()))
            .into_parts();
        assert_eq!(node.node, 6);
        assert_eq!(node.span, Region::for_file("spec.watsup"));
        assert_eq!(hints, Hints::default());
    }
}
