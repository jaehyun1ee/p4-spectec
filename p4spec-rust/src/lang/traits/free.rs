//! Free identifiers shared across language stages

use crate::lang::common::ds::set::IdSet;

/// Collects free term identifiers from syntax
pub trait Free {
    /// Returns the free term identifiers contained in `self`
    fn free(&self) -> IdSet;
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
