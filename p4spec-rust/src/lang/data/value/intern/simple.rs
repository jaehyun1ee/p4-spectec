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

/// Append-only storage sharing exactly equal items without cloning them
#[derive(Debug)]
pub struct Interner<T> {
    items: Vec<T>,
    table: HashTable<Entry>,
    hasher: RandomState,
}

// - Construction

impl<T> Default for Interner<T> {
    fn default() -> Self {
        Self {
            items: Vec::new(),
            table: HashTable::new(),
            hasher: RandomState::new(),
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
    /// Reuses an equal item or stores a new one, checking index overflow first
    pub fn intern(&mut self, item: T) -> Result<Interned<T>, TryFromIntError> {
        let hash = self.hasher.hash_one(&item);
        let index = if let Some(entry) = self.table.find(hash, |entry| {
            entry.hash == hash && self.items[entry.index as usize] == item
        }) {
            entry.index
        } else {
            let index = u32::try_from(self.items.len())?;
            self.items.push(item);
            self.table
                .insert_unique(hash, Entry { hash, index }, |entry| entry.hash);
            index
        };
        Ok(Interned {
            index,
            marker: PhantomData,
        })
    }
}
