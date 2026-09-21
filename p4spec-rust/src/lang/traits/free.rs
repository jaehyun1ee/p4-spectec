//! Free identifiers shared across language stages
//!
//! A type implements either `free` or `free_into`; each defaults to the other.
//! Blanket impls cover strings, spanned nodes, boxes, options, and slices.

use std::rc::Rc;

use crate::lang::common::{ds::set::IdSet, source::NotePhrase};

// == Free identifiers

/// Collects free term identifiers from syntax.
pub trait Free {
    /// Returns the free term identifiers contained in `self`
    fn free(&self) -> IdSet {
        let mut free = IdSet::new();
        self.free_into(&mut free);
        free
    }

    /// Adds the free term identifiers contained in `self` to `free`
    fn free_into(&self, free: &mut IdSet) {
        free.append(self.free());
    }
}

// - Text

impl Free for String {
    fn free(&self) -> IdSet {
        IdSet::new()
    }
}

// - Source annotations

impl<T: Free, N, S> Free for NotePhrase<T, N, S> {
    fn free_into(&self, free: &mut IdSet) {
        self.node.free_into(free);
    }
}

// - Containers

impl<T: Free + ?Sized> Free for Box<T> {
    fn free_into(&self, free: &mut IdSet) {
        self.as_ref().free_into(free);
    }
}

impl<T: Free + ?Sized> Free for Rc<T> {
    fn free_into(&self, free: &mut IdSet) {
        self.as_ref().free_into(free);
    }
}

impl<T: Free> Free for Option<T> {
    fn free_into(&self, free: &mut IdSet) {
        if let Some(value) = self {
            value.free_into(free);
        }
    }
}

impl<T: Free> Free for [T] {
    fn free_into(&self, free: &mut IdSet) {
        for item in self {
            item.free_into(free);
        }
    }
}
