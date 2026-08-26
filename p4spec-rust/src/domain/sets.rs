use std::collections::BTreeSet;

// Common set types

pub type IdSet = BTreeSet<String>;
pub type TIdSet = BTreeSet<String>;

// Set operations

/// Creates an empty set
pub fn empty<T>() -> BTreeSet<T> {
    BTreeSet::new()
}

/// Creates a singleton set
pub fn singleton<T: Ord>(item: T) -> BTreeSet<T> {
    BTreeSet::from([item])
}

/// Applies union
pub fn union<T: Ord>(mut set_l: BTreeSet<T>, set_r: BTreeSet<T>) -> BTreeSet<T> {
    set_l.extend(set_r);
    set_l
}

/// Applies unions
pub fn unions<T: Ord>(sets: impl IntoIterator<Item = BTreeSet<T>>) -> BTreeSet<T> {
    sets.into_iter().fold(empty(), union)
}
