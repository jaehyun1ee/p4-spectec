//! Free identifiers shared across language stages

use crate::lang::common::{ds::set::IdSet, source::Spanned};

/// Collects free term identifiers from syntax
pub trait Free {
    /// Adds the free term identifiers contained in `self` to `free`
    fn collect_free(&self, free: &mut IdSet);

    /// Returns the free term identifiers contained in `self`
    fn free(&self) -> IdSet {
        let mut free = IdSet::new();
        self.collect_free(&mut free);
        free
    }
}

impl Free for String {
    fn collect_free(&self, _free: &mut IdSet) {}
}

impl<T: Free> Free for Spanned<T> {
    fn collect_free(&self, free: &mut IdSet) {
        self.node.collect_free(free);
    }
}

impl<T: Free + ?Sized> Free for Box<T> {
    fn collect_free(&self, free: &mut IdSet) {
        self.as_ref().collect_free(free);
    }
}

impl<T: Free> Free for Option<T> {
    fn collect_free(&self, free: &mut IdSet) {
        if let Some(value) = self {
            value.collect_free(free);
        }
    }
}

impl<T: Free> Free for [T] {
    fn collect_free(&self, free: &mut IdSet) {
        for item in self {
            item.collect_free(free);
        }
    }
}
