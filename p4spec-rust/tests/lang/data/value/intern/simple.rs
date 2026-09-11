use std::{
    cell::Cell,
    collections::HashSet,
    hash::{Hash, Hasher},
    rc::Rc,
};

use p4spec_rust::lang::data::value::Interner;

use super::Collision;

#[derive(Debug)]
struct Counted {
    value: u32,
    clones: Rc<Cell<usize>>,
}

impl Clone for Counted {
    fn clone(&self) -> Self {
        self.clones.set(self.clones.get() + 1);
        Self {
            value: self.value,
            clones: self.clones.clone(),
        }
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
        0_u32.hash(hasher);
    }
}

#[test]
fn test_intern_ref_clones_only_missing_items_despite_collisions() {
    let mut interner = Interner::new();
    let clones = Rc::new(Cell::new(0));
    let item_l = Counted {
        value: 0,
        clones: clones.clone(),
    };
    let item_r = Counted {
        value: 1,
        clones: clones.clone(),
    };

    let id_l = interner.intern_ref(&item_l).unwrap();
    assert_eq!(clones.get(), 1);
    let id_r = interner.intern(item_r).unwrap();
    assert_eq!(clones.get(), 1);
    assert_ne!(id_l, id_r);

    assert_eq!(interner.intern_ref(&item_l).unwrap(), id_l);
    let item_r = Counted {
        value: 1,
        clones: clones.clone(),
    };
    assert_eq!(interner.intern_ref(&item_r).unwrap(), id_r);
    assert_eq!(interner.intern(item_l).unwrap(), id_l);
    assert_eq!(clones.get(), 1);
    assert_eq!(interner.get(id_l).value, 0);
    assert_eq!(interner.get(id_r).value, 1);
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
