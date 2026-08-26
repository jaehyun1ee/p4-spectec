//! Key policies shared by language collections

use std::{borrow::Borrow, cmp::Ordering};

use super::super::source::Spanned;

/// Selects the representation used to compare collection keys
pub trait CollectionKey {
    /// Representation whose ordering defines collection-key identity
    type Repr: Ord + ?Sized;

    /// Returns the representation used to compare this key
    fn repr(&self) -> &Self::Repr;
}

impl<T: Ord> CollectionKey for Spanned<T> {
    type Repr = T;

    fn repr(&self) -> &Self::Repr {
        &self.node
    }
}

#[repr(transparent)]
#[derive(Clone, Debug)]
pub(crate) struct ByKey<K: ?Sized>(pub(crate) K);

impl<T> Borrow<T> for ByKey<Spanned<T>> {
    fn borrow(&self) -> &T {
        &self.0.node
    }
}

impl<K: CollectionKey + ?Sized> PartialEq for ByKey<K> {
    fn eq(&self, key_other: &Self) -> bool {
        self.0.repr() == key_other.0.repr()
    }
}

impl<K: CollectionKey + ?Sized> Eq for ByKey<K> {}

impl<K: CollectionKey + ?Sized> PartialOrd for ByKey<K> {
    fn partial_cmp(&self, key_other: &Self) -> Option<Ordering> {
        Some(self.cmp(key_other))
    }
}

impl<K: CollectionKey + ?Sized> Ord for ByKey<K> {
    fn cmp(&self, key_other: &Self) -> Ordering {
        self.0.repr().cmp(key_other.0.repr())
    }
}
