//! Exact interning into stable handles
//!
//! Interning "x", "y", then "x" stores two strings
//! and returns the first handle again for the final "x".
//! Entries remain available until the interner drops.

use std::{
    hash::{BuildHasher, Hash},
    marker::PhantomData,
    num::TryFromIntError,
};

use foldhash::fast::RandomState;
use hashbrown::{Equivalent, HashTable};

use super::idx::Interned;

// = Interning storage

/// A table entry: the item's hash and its index in storage.
#[derive(Debug)]
struct Entry {
    hash: u64,
    index: u32,
}

/// Append-only storage sharing exactly equal items.
#[derive(Debug)]
pub struct Interner<T> {
    /// Items by handle index.
    items: Vec<T>,
    /// Hash table over `items`, keyed by item hash.
    table: HashTable<Entry>,
    /// Hasher for items.
    hasher: RandomState,
    /// Handle of `T::default()` once registered, checked before hashing.
    id_default: Option<Interned<T>>,
}

// - Construction

impl<T> Default for Interner<T> {
    fn default() -> Self {
        Self {
            items: Vec::new(),
            table: HashTable::new(),
            hasher: RandomState::default(),
            id_default: None,
        }
    }
}

impl<T> Interner<T> {
    /// An empty interner.
    pub fn new() -> Self {
        Self::default()
    }

    // - Lookup

    /// Borrows an item using a handle issued by this interner.
    pub fn get(&self, id: Interned<T>) -> &T {
        &self.items[id.index as usize]
    }
}

// - Interning

impl<T: Eq + Hash> Interner<T> {
    /// Registers the default value for reuse without hashing on later lookups.
    pub fn intern_default(&mut self) -> Result<Interned<T>, TryFromIntError>
    where
        T: Default,
    {
        // Register once
        if let Some(id) = self.id_default {
            return Ok(id);
        }
        let id = self.intern(T::default())?;
        self.id_default = Some(id);
        Ok(id)
    }

    /// Reuses an equal item or stores a new one, checking index overflow first.
    pub fn intern(&mut self, item: T) -> Result<Interned<T>, TryFromIntError> {
        self.intern_with(item, |item| item)
    }

    /// Looks up a query and constructs its stored item only when absent.
    ///
    /// The constructed item must be equivalent to the query and hash identically.
    /// Construction only converts the payload; it must have no observable effects.
    pub fn intern_with<Q: Hash + Equivalent<T>>(
        &mut self,
        query: Q,
        make: impl FnOnce(Q) -> T,
    ) -> Result<Interned<T>, TryFromIntError> {
        // The default item is checked by equality alone, without hashing
        if let Some(id) = self.id_default
            && query.equivalent(self.get(id))
        {
            return Ok(id);
        }
        // An equal item already stored shares its handle
        let hash = self.hasher.hash_one(&query);
        if let Some(id) = self.find(&query, hash) {
            return Ok(id);
        }
        // Check the next handle before materializing the owned payload
        let index = u32::try_from(self.items.len())?;
        self.insert(make(query), hash, index)
    }

    // - Table operations

    /// The handle of an equal stored item, if any.
    fn find<Q: Equivalent<T> + ?Sized>(&self, query: &Q, hash: u64) -> Option<Interned<T>> {
        self.table
            .find(hash, |entry| {
                entry.hash == hash && query.equivalent(&self.items[entry.index as usize])
            })
            .map(|entry| Interned { index: entry.index, marker: PhantomData })
    }

    /// Stores a new item at the next index.
    fn insert(&mut self, item: T, hash: u64, index: u32) -> Result<Interned<T>, TryFromIntError> {
        self.items.push(item);
        self.table
            .insert_unique(hash, Entry { hash, index }, |entry| entry.hash);
        Ok(Interned { index, marker: PhantomData })
    }
}
