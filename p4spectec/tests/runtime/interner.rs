use std::{
    cell::Cell,
    hash::{Hash, Hasher},
};

use hashbrown::Equivalent;
use p4spectec::lang::data::intern::{CanonEq, CanonHash, CanonInterner, Interner};

#[derive(Clone, Debug, Default, PartialEq, Eq)]
struct Entry {
    num: u32,
    class: u32,
}

impl Hash for Entry {
    fn hash<H: Hasher>(&self, hasher: &mut H) {
        0_u8.hash(hasher);
    }
}

struct Query(Entry);
impl Hash for Query {
    fn hash<H: Hasher>(&self, hasher: &mut H) {
        self.0.hash(hasher);
    }
}
impl Equivalent<Entry> for Query {
    fn equivalent(&self, entry: &Entry) -> bool {
        self.0 == *entry
    }
}
impl CanonEq for Entry {
    fn canon_eq(&self, _interner: &CanonInterner<Self>, _ctx: &(), entry: &Self) -> bool {
        self.class == entry.class
    }
}
impl CanonHash for Entry {
    fn canon_hash<H: Hasher>(&self, _interner: &CanonInterner<Self>, _ctx: &(), hasher: &mut H) {
        0_u8.hash(hasher);
    }
}

#[test]
fn exact_queries_resolve_collisions_and_construct_only_new_entries() {
    let mut interner = Interner::new();
    let id_default = interner.intern_default().unwrap();
    let count = Cell::new(0);
    let mut intern = |entry: Entry| {
        interner
            .intern_with(Query(entry), |query| {
                count.set(count.get() + 1);
                query.0
            })
            .unwrap()
    };
    assert_eq!(intern(Entry::default()), id_default);
    assert_eq!(count.get(), 0);
    let id_a = intern(Entry { num: 1, class: 3 });
    let id_b = intern(Entry { num: 2, class: 3 });
    assert_ne!(id_a, id_b);
    assert_eq!(intern(Entry { num: 1, class: 3 }), id_a);
    assert_eq!(intern(Entry { num: 2, class: 3 }), id_b);
    assert_eq!(count.get(), 2);
}

#[test]
fn canonical_queries_preserve_exact_and_canonical_insertion_order() {
    let mut interner = CanonInterner::new();
    let mut interner_reference = CanonInterner::new();
    let count = Cell::new(0);
    let mut ids = Vec::new();
    // All exact and canonical hashes collide, including unequal classes
    for (idx, entry) in [
        Entry { num: 1, class: 3 },
        Entry { num: 2, class: 3 },
        Entry { num: 3, class: 4 },
        Entry { num: 1, class: 3 },
        Entry { num: 2, class: 3 },
    ]
    .into_iter()
    .enumerate()
    {
        let id_reference = interner_reference.intern(entry.clone(), &()).unwrap();
        let id = if idx % 2 == 0 {
            interner
                .intern_with(Query(entry), &(), |query| {
                    count.set(count.get() + 1);
                    query.0
                })
                .unwrap()
        } else {
            interner.intern(entry, &()).unwrap()
        };
        assert_eq!(id, id_reference);
        assert_eq!(interner.canon_id(id), interner_reference.canon_id(id_reference));
        ids.push(id);
    }
    assert_ne!(ids[0], ids[1]);
    assert_eq!(interner.canon_id(ids[0]), interner.canon_id(ids[1]));
    assert_ne!(interner.canon_id(ids[0]), interner.canon_id(ids[2]));
    assert_eq!(count.get(), 2);
}

#[test]
fn canonical_hints_validate_exact_items_and_fall_back_on_collisions() {
    let mut interner = CanonInterner::new();
    let mut interner_reference = CanonInterner::new();
    let mut interner_foreign = Interner::new();
    let ids_foreign: Vec<_> = (0..12)
        .map(|num| interner_foreign.intern(Entry { num, class: 0 }).unwrap())
        .collect();
    let count = Cell::new(0);
    let mut ids = Vec::new();
    // Equal canonical classes and every hash collide, but exact items differ
    for (entry, hint) in [
        (Entry { num: 1, class: 3 }, Some(ids_foreign[11])),
        (Entry { num: 2, class: 3 }, Some(ids_foreign[0])),
        (Entry { num: 3, class: 4 }, None),
        (Entry { num: 1, class: 3 }, Some(ids_foreign[0])),
        (Entry { num: 2, class: 3 }, Some(ids_foreign[2])),
        (Entry { num: 3, class: 4 }, Some(ids_foreign[11])),
    ] {
        let id_reference = interner_reference.intern(entry.clone(), &()).unwrap();
        let id = interner
            .intern_with_hint(Query(entry), &(), hint, |query| {
                count.set(count.get() + 1);
                query.0
            })
            .unwrap();
        assert_eq!(id, id_reference);
        assert_eq!(interner.canon_id(id), interner_reference.canon_id(id_reference));
        ids.push(id);
    }
    assert_eq!(count.get(), 3);
    assert_ne!(ids[0], ids[1]);
    assert_eq!(interner.canon_id(ids[0]), interner.canon_id(ids[1]));
    assert_ne!(interner.canon_id(ids[0]), interner.canon_id(ids[2]));
}

impl CanonEq<Cell<bool>> for Entry {
    fn canon_eq(&self, _interner: &CanonInterner<Self>, _ctx: &Cell<bool>, entry: &Self) -> bool {
        self.class == entry.class
    }
}

impl CanonHash<Cell<bool>> for Entry {
    fn canon_hash<H: Hasher>(
        &self,
        _interner: &CanonInterner<Self>,
        ctx: &Cell<bool>,
        hasher: &mut H,
    ) {
        assert!(!ctx.replace(false), "interrupt canonical insertion");
        0_u8.hash(hasher);
    }
}

#[test]
fn canonical_hints_complete_an_entry_after_interrupted_canonical_insertion() {
    let mut interner = CanonInterner::new();
    let mut interner_reference = CanonInterner::new();
    let entry = Entry { num: 1, class: 3 };
    let id_reference = interner_reference.intern(entry.clone(), &()).unwrap();
    let ctx = Cell::new(true);
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        interner.intern(entry.clone(), &ctx)
    }));
    assert!(result.is_err());
    let id = interner
        .intern_with_hint(Query(entry), &ctx, Some(id_reference), |_| {
            panic!("the exact item already exists")
        })
        .unwrap();
    assert_eq!(id, id_reference);
    assert_eq!(interner.canon_id(id), interner_reference.canon_id(id_reference));
}
