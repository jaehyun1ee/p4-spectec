//! Maps that compare syntax keys without source or analysis metadata

use std::collections::BTreeMap;

use thiserror::Error;

use crate::lang::{common::Id, traits::cmp::SyntaxCmp};

use super::{collections::ByKey, set::PhraseSet};

/// A mismatch between the lengths of key and value lists
#[derive(Clone, Copy, Debug, Error, PartialEq, Eq)]
#[error("arity mismatch: expected {expected}, got {actual}")]
pub struct ArityMismatch {
    pub expected: usize,
    pub actual: usize,
}

impl ArityMismatch {
    pub const fn new(expected: usize, actual: usize) -> Self {
        Self { expected, actual }
    }
}

/// An ordered map that compares keys through `SyntaxCmp`
#[repr(transparent)]
#[derive(Clone, Debug)]
pub struct PhraseMap<K: SyntaxCmp, V> {
    entries: BTreeMap<ByKey<K>, V>,
}

impl<K: SyntaxCmp, V> PhraseMap<K, V> {
    /// Constructs an empty map
    pub fn new() -> Self {
        Self {
            entries: BTreeMap::new(),
        }
    }

    /// Constructs a map from equally sized key and value lists
    pub fn from_lists(keys: &[K], values: &[V]) -> Result<Self, ArityMismatch>
    where
        K: Clone,
        V: Clone,
    {
        if keys.len() != values.len() {
            let error = ArityMismatch::new(keys.len(), values.len());
            return Err(error);
        }

        let map = keys.iter().cloned().zip(values.iter().cloned()).collect();
        Ok(map)
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
    pub fn domain(&self) -> PhraseSet<K>
    where
        K: Clone,
    {
        self.keys().cloned().collect()
    }
}

impl<V> PhraseMap<Id, V> {
    /// Returns the value for an equivalent key
    pub fn get(&self, key: &Id) -> Option<&V> {
        self.entries.get(&key.node)
    }

    /// Returns the mutable value for an equivalent key
    pub fn get_mut(&mut self, key: &Id) -> Option<&mut V> {
        self.entries.get_mut(&key.node)
    }

    /// Returns whether an equivalent key is present
    pub fn contains_key(&self, key: &Id) -> bool {
        self.entries.contains_key(&key.node)
    }

    /// Removes and returns the value for an equivalent key
    pub fn remove(&mut self, key: &Id) -> Option<V> {
        self.entries.remove(&key.node)
    }

    /// Removes and returns the stored key and value for an equivalent key
    pub fn remove_entry(&mut self, key: &Id) -> Option<(Id, V)> {
        self.entries
            .remove_entry(&key.node)
            .map(|(key, value)| (key.0, value))
    }
}

impl<K: SyntaxCmp, V> Default for PhraseMap<K, V> {
    fn default() -> Self {
        Self::new()
    }
}

impl<K: SyntaxCmp, V> Extend<(K, V)> for PhraseMap<K, V> {
    fn extend<T: IntoIterator<Item = (K, V)>>(&mut self, bindings: T) {
        self.entries
            .extend(bindings.into_iter().map(|(key, value)| (ByKey(key), value)));
    }
}

impl<K: SyntaxCmp, V> FromIterator<(K, V)> for PhraseMap<K, V> {
    fn from_iter<T: IntoIterator<Item = (K, V)>>(bindings: T) -> Self {
        let mut map = Self::new();
        map.extend(bindings);
        map
    }
}

/// Map keyed by source-annotated identifiers
pub type IdMap<V> = PhraseMap<Id, V>;
