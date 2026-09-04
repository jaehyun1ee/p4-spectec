use p4spec_rust::{
    lang::{
        common::source::{Position, Span},
        data::value::make,
    },
    runtime::envs::caches::{CallCache, CallKey},
};

#[test]
fn test_call_cache_uses_structural_value_keys() {
    let value_a = make::bool(true, Span::default());
    let value_b = make::bool(
        true,
        Span::new(Position::new("other", 4, 0), Position::new("other", 4, 1)),
    );
    let mut cache: CallCache<&str> = CallCache::new();
    let key_a = CallKey::new("f", vec![value_a]);
    let key_b = CallKey::new("f", vec![value_b]);

    assert_eq!(cache.insert(key_a, "initial"), None);
    assert_eq!(cache.insert(key_b.clone(), "replacement"), None);
    assert_eq!(cache.get(&key_b), Some(&"replacement"));
    assert_eq!(cache.len(), 2);

    cache.clear();
    assert!(cache.is_empty());
}
