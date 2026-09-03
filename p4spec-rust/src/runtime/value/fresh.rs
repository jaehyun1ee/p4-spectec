//! Fresh type-variable generation scoped to a runtime operation

use crate::{
    lang::{
        common::source::Span,
        il::ast::{self, TypKind},
    },
    phrase,
};

// == Fresh type variables

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
        let name = format!("__FRESH{next}");
        let tparam = phrase!(node: name, span: Span::default());
        let typ_kind = TypKind::Var(tparam.clone(), Vec::new());
        let typ = phrase!(node: typ_kind, span: Span::default());
        (tparam, typ)
    }
}
