use crate::{
    domain::source::{HasSpan, Span},
    lang::{
        hints::{alter, fields},
        sl,
    },
};
#[derive(Clone, Debug, PartialEq, Default)]
pub struct Hints {
    pub prose: Option<alter::T>,
    pub prose_in: Option<alter::T>,
    pub prose_out: Option<alter::T>,
    pub prose_true: Option<alter::T>,
    pub prose_false: Option<alter::T>,
    pub prose_fields: Option<fields::T>,
    pub prose_input_exps: Option<Vec<sl::ast::Exp>>,
    pub prose_output_exps: Option<Vec<sl::ast::Exp>>,
}
pub fn empty() -> Hints {
    Hints::default()
}
#[derive(Clone, Debug, PartialEq)]
pub struct T<N> {
    pub node: N,
    pub hints: Hints,
}
pub fn no_hints<N>(node: N) -> T<N> {
    T {
        node,
        hints: empty(),
    }
}
impl<N: HasSpan> HasSpan for T<N> {
    fn span(&self) -> &Span {
        self.node.span()
    }
}
