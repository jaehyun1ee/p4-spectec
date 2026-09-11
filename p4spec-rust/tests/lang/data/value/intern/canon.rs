use std::{collections::HashSet, hash::Hash};

use p4spec_rust::lang::data::value::CanonInterner;

use super::Collision;

#[test]
fn test_canonical_interning_checks_collisions_and_preserves_original_items() {
    let mut interner = CanonInterner::new();
    let handles: Vec<_> = (0..512_u32)
        .map(|value| {
            interner
                .intern(
                    Collision(value),
                    |_, _, hasher| 0_u32.hash(hasher),
                    |item_l, item_r, _| item_l.0 / 2 == item_r.0 / 2,
                )
                .unwrap()
        })
        .collect();
    for (value, id) in handles.iter().copied().enumerate().rev() {
        let value = u32::try_from(value).unwrap();
        assert_eq!(interner.get(id), &Collision(value));
        let id_again = interner
            .intern(
                Collision(value),
                |_, _, hasher| 0_u32.hash(hasher),
                |item_l, item_r, _| item_l.0 / 2 == item_r.0 / 2,
            )
            .unwrap();
        assert_eq!(id, id_again);
    }
    for pair in handles.chunks_exact(2) {
        assert_ne!(pair[0], pair[1]);
        assert_eq!(interner.canon_id(pair[0]), interner.canon_id(pair[1]));
    }
    let ids_canon: HashSet<_> = handles.iter().map(|id| interner.canon_id(*id)).collect();
    assert_eq!(ids_canon.len(), 256);
}
