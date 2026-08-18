use p4spec_rust::{
    domain::source::Region,
    runtime::{
        dynamic::caches::{CallCache, CallKey, ClockCache},
        value::{ValueKind, make},
    },
};

fn span(file: &str) -> Region {
    Region::for_file(file)
}

#[test]
fn clock_cache_normalizes_capacity_and_updates_existing_keys() {
    let mut cache = ClockCache::new(0);
    assert_eq!(cache.capacity(), 1);
    assert!(cache.is_empty());

    cache.insert("key".to_owned(), 1);
    cache.insert("key".to_owned(), 2);

    assert_eq!(cache.len(), 1);
    assert_eq!(cache.find(&"key".to_owned()), Some(&2));
}

#[test]
fn clock_hand_gives_recent_hits_an_extra_sweep() {
    let mut cache = ClockCache::new(3);
    cache.insert("a", 1);
    cache.insert("b", 2);
    cache.insert("c", 3);

    cache.insert("d", 4);
    assert_eq!(cache.find(&"a"), None);
    assert_eq!(cache.find(&"b"), Some(&2));

    cache.insert("e", 5);
    assert_eq!(cache.find(&"b"), Some(&2));
    assert_eq!(cache.find(&"c"), None);
    assert_eq!(cache.find(&"d"), Some(&4));
    assert_eq!(cache.find(&"e"), Some(&5));
    assert_eq!(cache.len(), 3);
}

#[test]
fn clear_removes_written_slots_and_restarts_initial_fill() {
    let mut cache = ClockCache::new(2);
    cache.insert(1, "one");
    cache.insert(2, "two");
    cache.clear();

    assert!(cache.is_empty());
    assert_eq!(cache.find(&1), None);
    assert_eq!(cache.find(&2), None);

    cache.insert(3, "three");
    cache.insert(4, "four");
    assert_eq!(cache.find(&3), Some(&"three"));
    assert_eq!(cache.find(&4), Some(&"four"));
}

#[test]
fn call_cache_uses_semantic_runtime_values_without_vid_or_vhash() {
    let value_a = make::bool(true, span("left"));
    let value_b = make::new(
        ValueKind::BoolV(true),
        p4spec_rust::lang::il::ast::TypKind::TextT,
        span("right"),
    );
    let key_a: CallKey = ("function".to_owned(), vec![value_a]);
    let key_b: CallKey = ("function".to_owned(), vec![value_b]);
    let mut cache: CallCache<&str> = CallCache::new(8);

    cache.insert(key_a, "first");
    cache.insert(key_b.clone(), "second");

    assert_eq!(cache.len(), 1);
    assert_eq!(cache.find(&key_b), Some(&"second"));
}
