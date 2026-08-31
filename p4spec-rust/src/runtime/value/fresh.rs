//! Fresh type-variable generation scoped to a runtime operation

use crate::{
    lang::{
        common::source::Span,
        il::ast::{self, TypKind},
    },
    phrase,
};

/// Caller-owned source of fresh runtime type variables.
///
/// Separate values intentionally produce the same sequence, which keeps
/// independent runtime operations deterministic without process-global state.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Fresh {
    next: u64,
}

impl Fresh {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn fresh(&mut self) -> (ast::TParam, ast::Typ) {
        let next = self.next;
        self.next += 1;
        let tparam = phrase!(node: format!("__FRESH{next}"), span: Span::default());
        let typ_kind = TypKind::Var(tparam.clone(), vec![]);
        let typ = phrase!(node: typ_kind, span: Span::default());
        (tparam, typ)
    }
}
