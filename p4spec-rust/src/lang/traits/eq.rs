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

impl<T: SyntaxEq> SyntaxEq for Spanned<T> {
    fn syntax_eq(&self, other: &Self) -> bool {
        self.node.syntax_eq(&other.node)
    }
}

impl SyntaxEq for ExternalData {
    fn syntax_eq(&self, other: &Self) -> bool {
        self == other
    }
}

#[cfg(test)]
mod tests {
    use super::SyntaxEq;

    struct SyntaxNode(u8);

    impl SyntaxEq for SyntaxNode {
        fn syntax_eq(&self, other: &Self) -> bool {
            self.0 == other.0
        }
    }

    #[test]
    fn slices_compare_elements_in_order_by_default() {
        let nodes_l = [SyntaxNode(0), SyntaxNode(1)];
        let nodes_r = [SyntaxNode(0), SyntaxNode(1)];
        let nodes_changed = [SyntaxNode(1), SyntaxNode(0)];

        assert!(nodes_l.as_slice().syntax_eq(nodes_r.as_slice()));
        assert!(!nodes_l.as_slice().syntax_eq(nodes_changed.as_slice()));
    }
}
