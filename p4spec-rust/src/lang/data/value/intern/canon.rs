use std::{
    collections::hash_map::RandomState,
    fmt,
    hash::{BuildHasher, Hash, Hasher},
    num::TryFromIntError,
};

use hashbrown::HashTable;

use super::{idx::Interned, simple::Interner};

// = Canonical identities

/// A canonical identity valid only in the interner that issued it
#[repr(transparent)]
pub struct CanonId<T>(Interned<T>);

// - Copying

impl<T> Copy for CanonId<T> {}

impl<T> Clone for CanonId<T> {
    fn clone(&self) -> Self {
        *self
    }
}

// - Printing

impl<T> fmt::Debug for CanonId<T> {
    fn fmt(&self, fmt: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt.debug_tuple("CanonId").field(&self.0.index).finish()
    }
}

// - Equality

impl<T> PartialEq for CanonId<T> {
    fn eq(&self, id_other: &Self) -> bool {
        self.0 == id_other.0
    }
}

impl<T> Eq for CanonId<T> {}

// - Hashing

impl<T> Hash for CanonId<T> {
    fn hash<H: Hasher>(&self, hasher: &mut H) {
        self.0.hash(hasher);
    }
}

// = Canonical interning

#[derive(Debug)]
struct CanonEntry<T> {
    hash: u64,
    representative: Interned<T>,
}

/// Preserves exact items while sharing identities under a coarser equality
#[derive(Debug)]
pub struct CanonInterner<T> {
    storage: Interner<T>,
    canon: Vec<CanonId<T>>,
    canon_table: HashTable<CanonEntry<T>>,
    canon_hasher: RandomState,
}

// - Construction

impl<T> Default for CanonInterner<T> {
    fn default() -> Self {
        Self {
            storage: Interner::new(),
            canon: Vec::new(),
            canon_table: HashTable::new(),
            canon_hasher: RandomState::new(),
        }
    }
}

impl<T> CanonInterner<T> {
    pub fn new() -> Self {
        Self::default()
    }

    // - Lookup

    pub fn get(&self, id: Interned<T>) -> &T {
        self.storage.get(id)
    }

    pub fn canon_id(&self, id: Interned<T>) -> CanonId<T> {
        self.canon[id.index as usize]
    }
}

// - Interning

impl<T: Eq + Hash> CanonInterner<T> {
    /// Use consistent hash/equality rules across insertions
    ///
    /// Exact equality must imply canonical equality; referenced children must
    /// already have canonical identities in this interner
    pub fn intern(
        &mut self,
        item: T,
        hash_canon: impl FnOnce(&T, &Self, &mut <RandomState as BuildHasher>::Hasher),
        eq_canon: impl Fn(&T, &T, &Self) -> bool,
    ) -> Result<Interned<T>, TryFromIntError> {
        let id = self.storage.intern(item)?;
        if (id.index as usize) < self.canon.len() {
            return Ok(id);
        }
        let item = self.storage.get(id);
        let mut hasher = self.canon_hasher.build_hasher();
        hash_canon(item, self, &mut hasher);
        let hash = hasher.finish();
        let id_canon = self
            .canon_table
            .find(hash, |entry| {
                entry.hash == hash && eq_canon(item, self.get(entry.representative), self)
            })
            .map(|entry| CanonId(entry.representative));
        self.canon.push(id_canon.unwrap_or(CanonId(id)));
        if id_canon.is_none() {
            self.canon_table.insert_unique(
                hash,
                CanonEntry {
                    hash,
                    representative: id,
                },
                |entry| entry.hash,
            );
        }
        Ok(id)
    }
}
