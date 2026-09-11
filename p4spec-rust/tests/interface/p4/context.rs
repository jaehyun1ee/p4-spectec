use p4spec_rust::interface::p4::context::{Context, IdentKind, TypeId};
use p4spec_rust::lang::data::value::ValueArena;

#[test]
fn test_scopes_shadow_and_restore_identifier_kinds() {
    let mut arena = ValueArena::new();
    let context = Context::new(&mut arena);
    context.declare_typ("T", false).unwrap();
    context.scope_push();
    context
        .declare_var("T", false, TypeId::Local("U".to_owned()))
        .unwrap();

    assert!(matches!(context.ident_kind("T"), IdentKind::Ident { .. }));
    context.scope_pop().unwrap();
    assert!(matches!(
        context.ident_kind("T"),
        IdentKind::TypeName {
            has_params: false,
            ..
        }
    ));
}

#[test]
fn test_parent_namespace_classifies_members_without_global_state() {
    let mut arena = ValueArena::new();
    let context = Context::new(&mut arena);
    context.declare_typ("Header", false).unwrap();
    context.scope_push();
    context.declare_typ("FieldType", true).unwrap();
    let namespace = context.scope_pop().unwrap();
    context.namespace_set_typ("Header", namespace);
    context
        .declare_var("header", false, TypeId::Local("Header".to_owned()))
        .unwrap();

    context.ident_kind("header");
    context.namespace_set_parent();
    assert!(matches!(
        context.ident_kind("FieldType"),
        IdentKind::TypeName {
            has_params: true,
            ..
        }
    ));
    context.namespace_clear_parent();
    assert!(matches!(
        context.ident_kind("FieldType"),
        IdentKind::Ident { .. }
    ));
}

#[test]
fn test_contexts_are_isolated() {
    let mut arena = ValueArena::new();
    let context_a = Context::new(&mut arena);
    let mut arena_b = ValueArena::new();
    let context_b = Context::new(&mut arena_b);
    context_a.declare_typ("T", false).unwrap();

    assert!(matches!(
        context_a.ident_kind("T"),
        IdentKind::TypeName { .. }
    ));
    assert!(matches!(context_b.ident_kind("T"), IdentKind::Ident { .. }));
}

#[test]
fn test_go_local_discards_scopes_created_while_locals_are_suspended() {
    let mut arena = ValueArena::new();
    let context = Context::new(&mut arena);
    context.scope_push();
    context.declare_typ("Local", false).unwrap();
    context.scope_to_toplevel().unwrap();
    context.scope_push();
    context.declare_typ("Temporary", false).unwrap();

    context.scope_to_local();

    assert!(matches!(
        context.ident_kind("Local"),
        IdentKind::TypeName { .. }
    ));
    assert!(matches!(
        context.ident_kind("Temporary"),
        IdentKind::Ident { .. }
    ));
}
