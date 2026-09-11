//! Interning by Rc allocation identity
//!
//! An Rc and its clone share a handle; a fresh Rc with equal contents gets a
//! distinct handle. Retaining each allocation prevents its address from being
//! reused while the interner is alive.

use std::{
    collections::{HashMap, hash_map::Entry},
    marker::PhantomData,
    num::TryFromIntError,
    rc::Rc,
};

use super::idx::Interned;

// = Interning storage

/// Shares Rc allocations by address and retains them until the interner is dropped
#[derive(Debug)]
pub struct RcInterner<T> {
    items: Vec<Rc<T>>,
    table: HashMap<*const T, Interned<T>>,
}

// - Construction

impl<T> Default for RcInterner<T> {
    fn default() -> Self {
        Self {
            items: Vec::new(),
            table: HashMap::new(),
        }
    }
}

impl<T> RcInterner<T> {
    pub fn new() -> Self {
        Self::default()
    }

    // - Lookup

    pub fn get(&self, id: Interned<T>) -> &Rc<T> {
        &self.items[id.index as usize]
    }

    // - Interning

    /// Reuses an allocation without hashing or comparing its contents
    pub fn intern(&mut self, item: Rc<T>) -> Result<Interned<T>, TryFromIntError> {
        match self.table.entry(Rc::as_ptr(&item)) {
            Entry::Occupied(entry) => Ok(*entry.get()),
            Entry::Vacant(entry) => {
                let index = u32::try_from(self.items.len())?;
                let id = Interned {
                    index,
                    marker: PhantomData,
                };
                self.items.push(item);
                entry.insert(id);
                Ok(id)
            }
        }
    }
}
