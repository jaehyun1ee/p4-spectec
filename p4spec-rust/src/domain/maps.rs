use std::collections::{BTreeMap, BTreeSet};

/// Applies keys
pub fn keys<K: Clone + Ord, V>(map: &BTreeMap<K, V>) -> Vec<K> {
    map.keys().cloned().collect()
}

/// Applies domain
pub fn domain<K: Clone + Ord, V>(map: &BTreeMap<K, V>) -> BTreeSet<K> {
    map.keys().cloned().collect()
}

/// Applies values
pub fn values<K: Ord, V>(map: &BTreeMap<K, V>) -> Vec<&V> {
    map.values().collect()
}

/// Applies extend
pub fn extend<K: Ord, V>(mut map_l: BTreeMap<K, V>, map_r: BTreeMap<K, V>) -> BTreeMap<K, V> {
    map_l.extend(map_r);
    map_l
}

/// Applies diff
pub fn diff<K: Ord, V, W>(mut map_l: BTreeMap<K, V>, map_r: &BTreeMap<K, W>) -> BTreeMap<K, V> {
    map_l.retain(|key, _| !map_r.contains_key(key));
    map_l
}

/// Applies subset
pub fn subset<K: Ord, V, W>(
    map_l: &BTreeMap<K, V>,
    map_r: &BTreeMap<K, W>,
    mut equal_value: impl FnMut(&V, &W) -> bool,
) -> bool {
    map_l.iter().all(|(key, value_l)| {
        map_r
            .get(key)
            .is_some_and(|value_r| equal_value(value_l, value_r))
    })
}

/// Checks equality of ual
pub fn equal<K: Ord, V>(
    map_l: &BTreeMap<K, V>,
    map_r: &BTreeMap<K, V>,
    equal_value: impl Fn(&V, &V) -> bool,
) -> bool {
    map_l.len() == map_r.len()
        && map_l.iter().all(|(key, value_l)| {
            map_r
                .get(key)
                .is_some_and(|value_r| equal_value(value_l, value_r))
        })
}
