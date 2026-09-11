//! Exact interning into stable handles
//!
//! Interning "x", "y", then "x" stores two strings and returns the first handle
//! again for the final "x". Entries remain available until the interner drops.

use std::{
    collections::hash_map::RandomState,
    hash::{BuildHasher, Hash},
    marker::PhantomData,
    num::TryFromIntError,
};

use hashbrown::HashTable;

use super::idx::Interned;

// = Interning storage

#[derive(Debug)]
struct Entry {
    hash: u64,
    index: u32,
}

/// Append-only storage sharing exactly equal items
#[derive(Debug)]
pub struct Interner<T> {
    items: Vec<T>,
    table: HashTable<Entry>,
    hasher: RandomState,
    id_default: Option<Interned<T>>,
}

// - Construction

impl<T> Default for Interner<T> {
    fn default() -> Self {
        Self {
            items: Vec::new(),
            table: HashTable::new(),
            hasher: RandomState::new(),
            id_default: None,
        }
    }
}

impl<T> Interner<T> {
    pub fn new() -> Self {
        Self::default()
    }

    // - Lookup

    /// Borrows an item using a handle issued by this interner
    pub fn get(&self, id: Interned<T>) -> &T {
        &self.items[id.index as usize]
    }
}

// - Interning

impl<T: Eq + Hash> Interner<T> {
    /// Registers the default value for reuse without hashing on later lookups
    pub fn intern_default(&mut self) -> Result<Interned<T>, TryFromIntError>
    where
        T: Default,
    {
        if let Some(id) = self.id_default {
            return Ok(id);
        }
        let id = self.intern(T::default())?;
        self.id_default = Some(id);
        Ok(id)
    }

    /// Reuses an equal item or stores a new one, checking index overflow first
    pub fn intern(&mut self, item: T) -> Result<Interned<T>, TryFromIntError> {
        if let Some(id) = self.id_default
            && self.get(id) == &item
        {
            return Ok(id);
        }
        let hash = self.hasher.hash_one(&item);
        if let Some(id) = self.find(&item, hash) {
            return Ok(id);
        }
        self.insert(item, hash)
    }

    /// Clones a borrowed item only when it is not already stored
    pub fn intern_ref(&mut self, item: &T) -> Result<Interned<T>, TryFromIntError>
    where
        T: Clone,
    {
        if let Some(id) = self.id_default
            && self.get(id) == item
        {
            return Ok(id);
        }
        let hash = self.hasher.hash_one(item);
        if let Some(id) = self.find(item, hash) {
            return Ok(id);
        }
        self.insert(item.clone(), hash)
    }

    // - Table operations

    fn find(&self, item: &T, hash: u64) -> Option<Interned<T>> {
        self.table
            .find(hash, |entry| {
                entry.hash == hash && &self.items[entry.index as usize] == item
            })
            .map(|entry| Interned {
                index: entry.index,
                marker: PhantomData,
            })
    }

    fn insert(&mut self, item: T, hash: u64) -> Result<Interned<T>, TryFromIntError> {
        let index = u32::try_from(self.items.len())?;
        self.items.push(item);
        self.table
            .insert_unique(hash, Entry { hash, index }, |entry| entry.hash);
        Ok(Interned {
            index,
            marker: PhantomData,
        })
    }
}
