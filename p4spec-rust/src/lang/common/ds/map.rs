//! Maps that compare source-annotated keys without their spans

use std::collections::BTreeMap;

use crate::{domain::source::Spanned, lang::common::Id};

use super::{
    collections::{ByKey, CollectionKey},
    set::SpannedSet,
};

/// An ordered map that compares keys through `CollectionKey`
#[repr(transparent)]
#[derive(Clone, Debug)]
pub struct SpannedMap<K: CollectionKey, V> {
    entries: BTreeMap<ByKey<K>, V>,
}

impl<K: CollectionKey, V> SpannedMap<K, V> {
    /// Constructs an empty map
    pub fn new() -> Self {
        Self {
            entries: BTreeMap::new(),
        }
    }

    /// Returns whether the map contains no bindings
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Returns the number of bindings
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Removes all bindings
    pub fn clear(&mut self) {
        self.entries.clear();
    }

    /// Inserts a binding and returns the previous value
    pub fn insert(&mut self, key: K, value: V) -> Option<V> {
        self.entries.insert(ByKey(key), value)
    }

    /// Iterates over bindings in collection order
    pub fn iter(&self) -> impl Iterator<Item = (&K, &V)> {
        self.entries.iter().map(|(key, value)| (&key.0, value))
    }

    /// Iterates mutably over bindings in collection order
    pub fn iter_mut(&mut self) -> impl Iterator<Item = (&K, &mut V)> {
        self.entries.iter_mut().map(|(key, value)| (&key.0, value))
    }

    /// Iterates over keys in collection order
    pub fn keys(&self) -> impl Iterator<Item = &K> {
        self.entries.keys().map(|key| &key.0)
    }

    /// Iterates over values in collection order
    pub fn values(&self) -> impl Iterator<Item = &V> {
        self.entries.values()
    }

    /// Returns the set of stored keys
    pub fn domain(&self) -> SpannedSet<K>
    where
        K: Clone,
    {
        self.keys().cloned().collect()
    }
}

impl<T: Ord, V> SpannedMap<Spanned<T>, V> {
    /// Returns the value for an equivalent key
    pub fn get(&self, key: &Spanned<T>) -> Option<&V> {
        self.entries.get(&key.node)
    }

    /// Returns the mutable value for an equivalent key
    pub fn get_mut(&mut self, key: &Spanned<T>) -> Option<&mut V> {
        self.entries.get_mut(&key.node)
    }

    /// Returns whether an equivalent key is present
    pub fn contains_key(&self, key: &Spanned<T>) -> bool {
        self.entries.contains_key(&key.node)
    }

    /// Removes and returns the value for an equivalent key
    pub fn remove(&mut self, key: &Spanned<T>) -> Option<V> {
        self.entries.remove(&key.node)
    }

    /// Removes and returns the stored key and value for an equivalent key
    pub fn remove_entry(&mut self, key: &Spanned<T>) -> Option<(Spanned<T>, V)> {
        self.entries
            .remove_entry(&key.node)
            .map(|(key, value)| (key.0, value))
    }
}

impl<K: CollectionKey, V> Default for SpannedMap<K, V> {
    fn default() -> Self {
        Self::new()
    }
}

impl<K: CollectionKey, V> Extend<(K, V)> for SpannedMap<K, V> {
    fn extend<T: IntoIterator<Item = (K, V)>>(&mut self, bindings: T) {
        self.entries
            .extend(bindings.into_iter().map(|(key, value)| (ByKey(key), value)));
    }
}

impl<K: CollectionKey, V> FromIterator<(K, V)> for SpannedMap<K, V> {
    fn from_iter<T: IntoIterator<Item = (K, V)>>(bindings: T) -> Self {
        let mut map = Self::new();
        map.extend(bindings);
        map
    }
}

/// Map keyed by source-annotated identifiers
pub type IdMap<V> = SpannedMap<Id, V>;
