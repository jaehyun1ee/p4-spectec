use super::super::*;

#[test]
fn test_context_clone_isolates_local_bindings() {
    let id_free = id("free", 1);
    let context = Context::new();
    let mut context_local = context.clone();

    context_local.add_free(id_free.clone());

    assert!(!context.frees.contains(&id_free));
    assert!(context_local.frees.contains(&id_free));
}
