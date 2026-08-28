//! Syntax equality shared across language stages

use crate::{lang::common::source::Spanned, yojson::ExternalData};

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

impl<T, Rhs> SyntaxEq<Spanned<Rhs>> for Spanned<T>
where
    T: SyntaxEq<Rhs>,
{
    fn syntax_eq(&self, other: &Spanned<Rhs>) -> bool {
        self.node.syntax_eq(&other.node)
    }
}

impl<T, Rhs> SyntaxEq<Box<Rhs>> for Box<T>
where
    T: SyntaxEq<Rhs>,
{
    fn syntax_eq(&self, other: &Box<Rhs>) -> bool {
        self.as_ref().syntax_eq(other.as_ref())
    }
}

impl<T, Rhs> SyntaxEq<Option<Rhs>> for Option<T>
where
    T: SyntaxEq<Rhs>,
{
    fn syntax_eq(&self, other: &Option<Rhs>) -> bool {
        match (self, other) {
            (Some(value_l), Some(value_r)) => value_l.syntax_eq(value_r),
            (None, None) => true,
            _ => false,
        }
    }
}

impl SyntaxEq for ExternalData {
    fn syntax_eq(&self, other: &Self) -> bool {
        self == other
    }
}
