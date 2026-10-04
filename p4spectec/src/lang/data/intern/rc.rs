//! Interning by Rc allocation identity
//!
//! An Rc and its clone share a handle;
//! a fresh Rc with equal contents gets a distinct handle.
//! Deferred clones reserve distinct handles before their contents are read.
//! Retaining each materialized allocation prevents address reuse.

use std::{
    collections::{HashMap, hash_map::Entry},
    fmt,
    marker::PhantomData,
    num::TryFromIntError,
    rc::Rc,
    sync::{Mutex, OnceLock, PoisonError},
};

use foldhash::fast::RandomState;

use super::idx::Interned;

// = Interning storage

enum RcEntry<T> {
    Shared(Rc<T>),
    Deferred { template: Rc<T>, item: OnceLock<Rc<T>> },
}

/// Shares Rc allocations by address, retaining them until the interner drops.
pub struct RcInterner<T> {
    items: Vec<RcEntry<T>>,
    table: Mutex<HashMap<*const T, Interned<T>, RandomState>>,
    clone_item: Option<fn(&T) -> T>,
}

// - Construction

impl<T> Default for RcInterner<T> {
    fn default() -> Self {
        Self { items: Vec::new(), table: Mutex::new(HashMap::default()), clone_item: None }
    }
}

impl<T> RcInterner<T> {
    /// An empty interner.
    pub fn new() -> Self {
        Self::default()
    }

    // - Lookup

    /// The allocation behind a handle, materialized once if it was deferred.
    pub fn get(&self, id: Interned<T>) -> &Rc<T> {
        match &self.items[id.index as usize] {
            // Ordinary allocations retain their original identity
            RcEntry::Shared(item) => item,
            // Each reserved entry materializes an independent allocation
            RcEntry::Deferred { template, item } => item.get_or_init(|| {
                let clone_item = self
                    .clone_item
                    .expect("deferred entries have a clone function");
                let item =
                    stacker::maybe_grow(64 * 1024, 1024 * 1024, || Rc::new(clone_item(template)));
                self.table
                    .lock()
                    .unwrap_or_else(PoisonError::into_inner)
                    .insert(Rc::as_ptr(&item), id);
                item
            }),
        }
    }

    // - Interning

    /// Reuses an allocation without hashing or comparing its contents.
    pub fn intern(&mut self, item: Rc<T>) -> Result<Interned<T>, TryFromIntError> {
        let table = self.table.get_mut().unwrap_or_else(PoisonError::into_inner);
        // A previously exposed allocation keeps its reserved handle
        match table.entry(Rc::as_ptr(&item)) {
            Entry::Occupied(entry) => Ok(*entry.get()),
            // A new allocation follows all eager and deferred entries
            Entry::Vacant(entry) => {
                let index = u32::try_from(self.items.len())?;
                let id = Interned { index, marker: PhantomData };
                self.items.push(RcEntry::Shared(item));
                entry.insert(id);
                Ok(id)
            }
        }
    }
}

impl<T: Clone> RcInterner<T> {
    /// Reserves a distinct allocation and clones its contents on first read.
    ///
    /// The template must remain immutable and cloning must have no other effects.
    /// Each handle materializes a fresh Rc, even for equal templates.
    pub fn intern_fresh_clone(&mut self, template: Rc<T>) -> Result<Interned<T>, TryFromIntError> {
        let index = u32::try_from(self.items.len())?;
        let id = Interned { index, marker: PhantomData };
        self.clone_item = Some(T::clone);
        self.items
            .push(RcEntry::Deferred { template, item: OnceLock::new() });
        Ok(id)
    }
}

impl<T: fmt::Debug> fmt::Debug for RcInterner<T> {
    fn fmt(&self, fmt: &mut fmt::Formatter<'_>) -> fmt::Result {
        struct Items<'a, T>(&'a RcInterner<T>);
        impl<T: fmt::Debug> fmt::Debug for Items<'_, T> {
            fn fmt(&self, fmt: &mut fmt::Formatter<'_>) -> fmt::Result {
                let mut items = fmt.debug_list();
                for idx in 0..self.0.items.len() {
                    let id = Interned { index: idx as u32, marker: PhantomData };
                    items.entry(self.0.get(id));
                }
                items.finish()
            }
        }
        struct Table<'a, T>(&'a [(*const T, Interned<T>)]);
        impl<T> fmt::Debug for Table<'_, T> {
            fn fmt(&self, fmt: &mut fmt::Formatter<'_>) -> fmt::Result {
                fmt.debug_map()
                    .entries(self.0.iter().map(|(ptr, id)| (ptr, id)))
                    .finish()
            }
        }
        // Display every allocation before displaying the complete pointer table
        let mut interner = fmt.debug_struct("RcInterner");
        interner.field("items", &Items(self));
        // User formatters can reenter without holding the pointer-table lock
        let table: Vec<_> = self
            .table
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .iter()
            .map(|(ptr, id)| (*ptr, *id))
            .collect();
        interner.field("table", &Table(&table));
        interner.finish()
    }
}
