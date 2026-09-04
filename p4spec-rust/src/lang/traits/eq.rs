//! Syntax equality shared across language stages

use std::rc::Rc;

use crate::{lang::common::source::NotePhrase, yojson::ExternalData};

/// Compares syntax while ignoring source and analysis metadata
pub trait SyntaxEq<Rhs: ?Sized = Self> {
    /// Returns whether two nodes represent the same syntax
    fn syntax_eq(&self, other: &Rhs) -> bool;

    /// Compares slices of nodes using this type's collection policy
    fn slice_syntax_eq(items_l: &[Self], items_r: &[Rhs]) -> bool
    where
        Self: Sized,
        Rhs: Sized,
    {
        items_l.len() == items_r.len()
            && items_l
                .iter()
                .zip(items_r)
                .all(|(item_l, item_r)| item_l.syntax_eq(item_r))
    }
}

impl<T, Rhs> SyntaxEq<[Rhs]> for [T]
where
    T: SyntaxEq<Rhs>,
{
    fn syntax_eq(&self, other: &[Rhs]) -> bool {
        T::slice_syntax_eq(self, other)
    }
}

impl SyntaxEq for String {
    fn syntax_eq(&self, other: &Self) -> bool {
        self == other
    }
}

impl<T: SyntaxEq, N> SyntaxEq for NotePhrase<T, N> {
    fn syntax_eq(&self, other: &Self) -> bool {
        self.node.syntax_eq(&other.node)
    }
}

impl<T: SyntaxEq + ?Sized> SyntaxEq for Rc<T> {
    fn syntax_eq(&self, other: &Self) -> bool {
        self.as_ref().syntax_eq(other.as_ref())
    }
}

impl SyntaxEq for ExternalData {
    fn syntax_eq(&self, other: &Self) -> bool {
        self == other
    }
}
