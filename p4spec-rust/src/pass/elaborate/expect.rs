//! Expected types and declaration context for elaboration
//!
//! Expression checks carry the current type separately from its declaration.
//! Unfolding an alias changes the current type while retaining that context.
//! Notation checks retain the complete declaration and its owner during recursion.

use crate::lang::{
    common::{Id, source::Span},
    il::ast as il,
};

/// Carries the current expected type and the check that introduced it.
#[derive(Clone, Copy)]
pub(super) struct ExpExpect<'a> {
    /// Holds the type used by the current checking rule.
    pub typ_il: &'a il::Typ,
    /// Retains the declaration context through type unfolding.
    pub kind: ExpExpectKind<'a>,
}

impl<'a> ExpExpect<'a> {
    /// Creates an expectation without declaration context.
    pub(super) fn plain(typ_il: &'a il::Typ) -> Self {
        Self { typ_il, kind: ExpExpectKind::Plain }
    }

    /// Replaces the checking type while retaining the declaration context.
    pub(super) fn with_typ<'b>(&'b self, typ_il: &'b il::Typ) -> ExpExpect<'b> {
        ExpExpect { typ_il, kind: self.kind }
    }
}

/// Identifies the source of an expression's expected type.
#[derive(Clone, Copy)]
pub(super) enum ExpExpectKind<'a> {
    /// Checks an expression without a corresponding declaration slot.
    Plain,
    /// Counts notation argument slots from zero, excluding literal tokens.
    NotArg {
        idx: usize,
        not_kind: NotExpectKind<'a>,
        typ_decl_il: &'a il::Typ,
        span_declaration: &'a Span,
    },
    /// Counts function parameters from zero after call-site instantiation.
    FuncArg { idx: usize, id_func: &'a Id, typ_decl_il: &'a il::Typ, span_declaration: &'a Span },
    /// Checks a function body against the declared return type.
    FuncReturn { id_func: &'a Id, typ_decl_il: &'a il::Typ, span_declaration: &'a Span },
}

/// Retains a complete notation declaration during subtree matching.
#[derive(Clone, Copy)]
pub(super) struct NotExpect<'a> {
    /// Holds the complete notation, independently of the current subtree.
    pub not_typ_il: &'a il::NotTyp,
    /// Identifies the declaration that owns the notation.
    pub kind: NotExpectKind<'a>,
}

/// Distinguishes relation notation from variant case notation.
#[derive(Clone, Copy)]
pub(super) enum NotExpectKind<'a> {
    /// Names the relation being checked.
    Relation(&'a Id),
    /// Checks a case of a variant type.
    Variant,
}
