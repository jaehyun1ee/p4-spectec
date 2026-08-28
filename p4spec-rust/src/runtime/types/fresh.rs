//! Fresh type-variable generation scoped to a runtime operation

use crate::lang::il::ast::{self, TypKind};
use crate::spanned_default;

#[derive(Default)]
pub(crate) struct Fresh {
    next: u64,
}

impl Fresh {
    pub(crate) fn fresh(&mut self) -> (ast::TParam, ast::Typ) {
        let next = self.next;
        self.next += 1;
        let tparam = spanned_default!(node: format!("__FRESH{next}"));
        let typ_kind = TypKind::Var(tparam.clone(), vec![]);
        let typ = spanned_default!(node: typ_kind);
        (tparam, typ)
    }
}
