//! Sets that compare source-annotated keys without their spans

use std::collections::BTreeSet;

use crate::{domain::source::Spanned, lang::common::Id};

use super::collections::{ByKey, CollectionKey};

/// An ordered set that compares keys through `CollectionKey`
#[repr(transparent)]
#[derive(Clone, Debug)]
pub struct SpannedSet<K: CollectionKey> {
    entries: BTreeSet<ByKey<K>>,
}

impl<K: CollectionKey> SpannedSet<K> {
    /// Constructs an empty set
    pub fn new() -> Self {
        Self {
            entries: BTreeSet::new(),
        }
    }

    /// Returns whether the set contains no keys
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Returns the number of keys
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Inserts a key, ignoring differences outside its collection representation
    pub fn insert(&mut self, key: K) -> bool {
        self.entries.insert(ByKey(key))
    }

    /// Returns the union with `set_other`
    pub fn union(mut self, set_other: Self) -> Self {
        self.entries.extend(set_other.entries);
        self
    }

    /// Iterates over stored keys in collection order
    pub fn iter(&self) -> impl Iterator<Item = &K> {
        self.entries.iter().map(|key| &key.0)
    }
}

impl<T: Ord> SpannedSet<Spanned<T>> {
    /// Returns whether an equivalent key is present
    pub fn contains(&self, key: &Spanned<T>) -> bool {
        self.entries.contains(&key.node)
    }

    /// Returns the stored key equivalent to `key`
    pub fn get(&self, key: &Spanned<T>) -> Option<&Spanned<T>> {
        self.entries.get(&key.node).map(|key| &key.0)
    }

    /// Removes and returns the stored key equivalent to `key`
    pub fn take(&mut self, key: &Spanned<T>) -> Option<Spanned<T>> {
        self.entries.take(&key.node).map(|key| key.0)
    }
}

impl<K: CollectionKey> Default for SpannedSet<K> {
    fn default() -> Self {
        Self::new()
    }
}

impl<K: CollectionKey> PartialEq for SpannedSet<K> {
    fn eq(&self, set_other: &Self) -> bool {
        self.entries == set_other.entries
    }
}

impl<K: CollectionKey> Eq for SpannedSet<K> {}

impl<K: CollectionKey> Extend<K> for SpannedSet<K> {
    fn extend<T: IntoIterator<Item = K>>(&mut self, keys: T) {
        self.entries.extend(keys.into_iter().map(ByKey));
    }
}

impl<K: CollectionKey> FromIterator<K> for SpannedSet<K> {
    fn from_iter<T: IntoIterator<Item = K>>(keys: T) -> Self {
        let mut set = Self::new();
        set.extend(keys);
        set
    }
}

impl<K: CollectionKey, const N: usize> From<[K; N]> for SpannedSet<K> {
    fn from(keys: [K; N]) -> Self {
        keys.into_iter().collect()
    }
}

/// Set of source-annotated identifiers
pub type IdSet = SpannedSet<Id>;

/// Set of source-annotated type identifiers
pub type TIdSet = IdSet;
