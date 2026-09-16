use std::{
    cell::Cell,
    collections::HashSet,
    hash::{Hash, Hasher},
    rc::Rc,
};

use p4spec_rust::lang::data::value::Interner;

use super::Collision;

#[derive(Debug, Default)]
struct Counted {
    value: u32,
    clones: Rc<Cell<usize>>,
    hashes: Rc<Cell<usize>>,
}

impl Clone for Counted {
    fn clone(&self) -> Self {
        self.clones.set(self.clones.get() + 1);
        Self { value: self.value, clones: self.clones.clone(), hashes: self.hashes.clone() }
    }
}

impl PartialEq for Counted {
    fn eq(&self, other: &Self) -> bool {
        self.value == other.value
    }
}

impl Eq for Counted {}

impl Hash for Counted {
    fn hash<H: Hasher>(&self, hasher: &mut H) {
        self.hashes.set(self.hashes.get() + 1);
        0_u32.hash(hasher);
    }
}

#[test]
fn test_intern_preserves_values_and_handles_through_collisions_and_growth() {
    let mut interner = Interner::new();
    let handles: Vec<_> = (0..512)
        .map(|value| interner.intern(Collision(value)).unwrap())
        .collect();
    let handles_unique: HashSet<_> = handles.iter().copied().collect();

    assert_eq!(handles_unique.len(), 512);
    for (value, handle) in handles.into_iter().enumerate().rev() {
        let value = u32::try_from(value).unwrap();
        assert_eq!(interner.get(handle), &Collision(value));
        assert_eq!(interner.intern(Collision(value)).unwrap(), handle);
        assert!(handles_unique.contains(&handle));
    }
}

#[test]
fn test_intern_preserves_values_and_handles_through_growth() {
    let mut interner = Interner::new();
    let handles: Vec<_> = (0..512_u32)
        .map(|value| interner.intern(value).unwrap())
        .collect();

    for (value, handle) in handles.into_iter().enumerate().rev() {
        let value = u32::try_from(value).unwrap();
        assert_eq!(interner.get(handle), &value);
        assert_eq!(interner.intern(value).unwrap(), handle);
    }
}

#[test]
fn test_intern_default_reuses_existing_handles_through_growth() {
    let mut interner = Interner::new();
    let id_other = interner.intern(7_u32).unwrap();
    let id_default = interner.intern(0).unwrap();
    assert_ne!(id_default, id_other);
    assert_eq!(interner.intern_default().unwrap(), id_default);
    for value in 1..512 {
        let id = interner.intern(value).unwrap();
        assert_ne!(id, id_default);
        assert_eq!(*interner.get(id), value);
    }
    assert_eq!(interner.intern_default().unwrap(), id_default);
    assert_eq!(interner.intern(0).unwrap(), id_default);
    assert_eq!(interner.intern(0).unwrap(), id_default);
    assert_eq!(interner.intern(7).unwrap(), id_other);
}

#[test]
fn test_registered_default_skips_hashing_and_cloning() {
    let mut interner = Interner::<Counted>::new();
    let id_default = interner.intern_default().unwrap();
    let item = interner.get(id_default).clone();
    let clones = item.clones.clone();
    let hashes = item.hashes.clone();
    clones.set(0);
    hashes.set(0);
    assert_eq!(interner.intern(item).unwrap(), id_default);
    assert_eq!(interner.intern_default().unwrap(), id_default);
    assert_eq!(clones.get(), 0);
    assert_eq!(hashes.get(), 0);
    let id_other = interner
        .intern(Counted { value: 1, ..Counted::default() })
        .unwrap();
    assert_ne!(id_other, id_default);
    assert_eq!(interner.get(id_default).value, 0);
    assert_eq!(interner.get(id_other).value, 1);
}
