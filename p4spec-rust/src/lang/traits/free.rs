//! Free identifiers shared across language stages

use crate::lang::common::{ds::set::IdSet, source::Spanned};

/// Collects free term identifiers from syntax
pub trait Free {
    /// Returns the free term identifiers contained in `self`
    fn free(&self) -> IdSet;
}

impl Free for String {
    fn free(&self) -> IdSet {
        IdSet::new()
    }
}

impl<T: Free> Free for Spanned<T> {
    fn free(&self) -> IdSet {
        self.node.free()
    }
}

impl<T: Free + ?Sized> Free for Box<T> {
    fn free(&self) -> IdSet {
        self.as_ref().free()
    }
}

impl<T: Free> Free for Option<T> {
    fn free(&self) -> IdSet {
        self.as_ref().map_or_else(IdSet::new, Free::free)
    }
}

impl<T: Free> Free for [T] {
    fn free(&self) -> IdSet {
        self.iter()
            .fold(IdSet::new(), |free, item| free.union(item.free()))
    }
}

#[cfg(test)]
mod tests {
    use crate::lang::common::{Id, source::Spanned};

    use super::{Free, IdSet};

    struct FreeNode(Id);

    impl Free for FreeNode {
        fn free(&self) -> IdSet {
            IdSet::from([self.0.clone()])
        }
    }

    #[test]
    fn spanned_free_delegates_to_the_node() {
        let id = Spanned::new("x".to_owned(), Default::default());
        let node = Spanned::new(FreeNode(id.clone()), Default::default());

        assert!(node.free().contains(&id));
    }
}
