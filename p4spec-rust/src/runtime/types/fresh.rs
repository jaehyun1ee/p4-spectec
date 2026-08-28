use std::cell::Cell;

use crate::lang::{
    common::source::{Span, Spanned},
    il::ast::{self, TypKind},
};

thread_local! {
    static NEXT_FRESH_TYPE_ID: Cell<u64> = const { Cell::new(0) };
}

#[derive(Default)]
pub(crate) struct FreshTypes;

impl FreshTypes {
    pub(crate) fn fresh(&mut self) -> (ast::TParam, ast::Typ) {
        let next = NEXT_FRESH_TYPE_ID.get();
        NEXT_FRESH_TYPE_ID.set(next + 1);
        let id = Spanned::new(format!("__FRESH{next}"), Span::default());
        let typ = Spanned::new(TypKind::Var(id.clone(), vec![]), Span::default());
        (id, typ)
    }
}

/// Resets fresh type identifiers for a new independent processing session
pub fn reset_fresh_type_ids() {
    NEXT_FRESH_TYPE_ID.set(0);
}
