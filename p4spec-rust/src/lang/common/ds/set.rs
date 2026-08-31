//! Sets that compare source-annotated keys without their spans

use std::collections::BTreeSet;

use crate::lang::common::{Id, source::Phrase};

use super::collections::{ByKey, CollectionKey};

/// An ordered set that compares keys through `CollectionKey`
#[repr(transparent)]
#[derive(Clone, Debug)]
pub struct PhraseSet<K: CollectionKey> {
    entries: BTreeSet<ByKey<K>>,
}

impl<K: CollectionKey> PhraseSet<K> {
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

impl<T: Ord> PhraseSet<Phrase<T>> {
    /// Returns whether an equivalent key is present
    pub fn contains(&self, key: &Phrase<T>) -> bool {
        self.entries.contains(&key.node)
    }

    /// Returns the stored key equivalent to `key`
    pub fn get(&self, key: &Phrase<T>) -> Option<&Phrase<T>> {
        self.entries.get(&key.node).map(|key| &key.0)
    }

    /// Removes and returns the stored key equivalent to `key`
    pub fn take(&mut self, key: &Phrase<T>) -> Option<Phrase<T>> {
        self.entries.take(&key.node).map(|key| key.0)
    }
}

impl<K: CollectionKey> Default for PhraseSet<K> {
    fn default() -> Self {
        Self::new()
    }
}

impl<K: CollectionKey> PartialEq for PhraseSet<K> {
    fn eq(&self, set_other: &Self) -> bool {
        self.entries == set_other.entries
    }
}

impl<K: CollectionKey> Eq for PhraseSet<K> {}

impl<K: CollectionKey> Extend<K> for PhraseSet<K> {
    fn extend<T: IntoIterator<Item = K>>(&mut self, keys: T) {
        self.entries.extend(keys.into_iter().map(ByKey));
    }
}

impl<K: CollectionKey> FromIterator<K> for PhraseSet<K> {
    fn from_iter<T: IntoIterator<Item = K>>(keys: T) -> Self {
        let mut set = Self::new();
        set.extend(keys);
        set
    }
}

impl<K: CollectionKey, const N: usize> From<[K; N]> for PhraseSet<K> {
    fn from(keys: [K; N]) -> Self {
        keys.into_iter().collect()
    }
}

/// Set of source-annotated identifiers
pub type IdSet = PhraseSet<Id>;

/// Set of source-annotated type identifiers
pub type TIdSet = IdSet;
