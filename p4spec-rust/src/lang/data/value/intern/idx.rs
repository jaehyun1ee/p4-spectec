use std::{
    fmt,
    hash::{Hash, Hasher},
    marker::PhantomData,
};

// = Typed handles

/// A compact handle valid only in the interner that issued it
#[repr(transparent)]
pub struct Interned<T> {
    pub(super) index: u32,
    pub(super) marker: PhantomData<fn() -> T>,
}

// - Copying

// Handle operations depend only on the index, not on traits of T
impl<T> Copy for Interned<T> {}

impl<T> Clone for Interned<T> {
    fn clone(&self) -> Self {
        *self
    }
}

// - Printing

impl<T> fmt::Debug for Interned<T> {
    fn fmt(&self, fmt: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt.debug_tuple("Interned").field(&self.index).finish()
    }
}

// - Equality

impl<T> PartialEq for Interned<T> {
    fn eq(&self, id_other: &Self) -> bool {
        self.index == id_other.index
    }
}

impl<T> Eq for Interned<T> {}

// - Hashing

impl<T> Hash for Interned<T> {
    fn hash<H: Hasher>(&self, hasher: &mut H) {
        self.index.hash(hasher);
    }
}
