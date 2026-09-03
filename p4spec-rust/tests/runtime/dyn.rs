use p4spec_rust::{
    lang::{
        common::source::{Position, Span},
        il::ast::Iter,
    },
    phrase,
    runtime::{
        r#dyn::{CallCache, CallKey, VEnv, Variable},
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
fn test_variables_ignore_identifier_spans_and_order_iterators() {
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
fn test_value_environment_iterates_deterministically_and_replaces_equivalent_keys() {
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
