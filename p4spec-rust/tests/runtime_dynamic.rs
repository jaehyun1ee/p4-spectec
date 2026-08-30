use p4spec_rust::{
    lang::{
        common::source::{Position, Span},
        il::ast::Iter,
    },
    phrase,
    runtime::{
        cache::ClockCache,
        dynamic::{CallCache, CallKey, VEnv, Variable},
        value::make,
    },
};

fn id(name: &str, line: i64) -> p4spec_rust::lang::il::ast::Id {
    let span = Span::new(
        Position::new("vars.watsup", line, 0),
        Position::new("vars.watsup", line, 1),
    );
    phrase!(node: name.to_owned(), span: span)
}

#[test]
fn variables_ignore_identifier_spans_and_order_iterators() {
    let plain_a = Variable::new(id("x", 1), vec![]);
    let plain_b = Variable::new(id("x", 9), vec![]);
    let optional = Variable::new(id("x", 2), vec![Iter::Opt]);
    let listed = Variable::new(id("x", 3), vec![Iter::List]);

    assert_eq!(plain_a, plain_b);
    assert!(plain_a < optional);
    assert!(optional < listed);
    assert_eq!(listed.to_string(), "x*");
}

#[test]
fn value_environment_iterates_deterministically_and_replaces_equivalent_keys() {
    let mut venv = VEnv::new();
    venv.insert(
        Variable::new(id("z", 1), vec![]),
        make::bool(false, Span::default()),
    );
    venv.insert(
        Variable::new(id("a", 2), vec![]),
        make::bool(true, Span::default()),
    );
    venv.insert(
        Variable::new(id("a", 8), vec![]),
        make::bool(false, Span::default()),
    );

    let names = venv
        .keys()
        .map(|variable| variable.id.node.as_str())
        .collect::<Vec<_>>();
    assert_eq!(names, ["a", "z"]);
    assert_eq!(venv.len(), 2);
}

#[test]
fn clock_cache_replaces_clears_and_uses_clock_eviction_order() {
    let mut cache = ClockCache::new(2);
    cache.insert("a", 1);
    cache.insert("b", 2);
    cache.insert("c", 3);

    assert_eq!(cache.find(&"a"), None);
    assert_eq!(cache.find(&"c"), Some(&3));

    cache.insert("d", 4);
    assert_eq!(cache.find(&"b"), None);
    assert_eq!(cache.find(&"c"), Some(&3));
    assert_eq!(cache.find(&"d"), Some(&4));

    cache.insert("c", 30);
    assert_eq!(cache.len(), 2);
    assert_eq!(cache.find(&"c"), Some(&30));

    cache.clear();
    assert!(cache.is_empty());
    assert_eq!(cache.capacity(), 2);
}

#[test]
fn zero_sized_cache_retains_one_entry() {
    let mut cache = ClockCache::new(0);
    cache.insert(1, "first");
    cache.insert(2, "second");

    assert_eq!(cache.capacity(), 1);
    assert_eq!(cache.len(), 1);
    assert_eq!(cache.find(&1), None);
    assert_eq!(cache.find(&2), Some(&"second"));
}

#[test]
fn call_keys_use_semantic_value_identity() {
    let value_a = make::bool(true, Span::default());
    let value_b = make::bool(
        true,
        Span::new(Position::new("other", 4, 0), Position::new("other", 4, 1)),
    );
    let mut cache: CallCache<&str> = CallCache::new(4);
    cache.insert(CallKey::new("f", vec![value_a]), "result");

    assert_eq!(
        cache.find(&CallKey::new("f", vec![value_b])),
        Some(&"result")
    );
}
