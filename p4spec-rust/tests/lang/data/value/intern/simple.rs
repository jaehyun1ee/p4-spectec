use std::collections::HashSet;

use p4spec_rust::lang::data::value::Interner;

use super::Collision;

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
