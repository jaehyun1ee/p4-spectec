use p4spec_rust::interface::p4::context::{Context, IdentKind, TypeId};
use p4spec_rust::lang::data::value::ValueArena;

#[test]
fn test_scopes_shadow_and_restore_identifier_kinds() {
    let mut arena = ValueArena::new();
    let ctx = Context::new(&mut arena);
    ctx.declare_typ("T", false).unwrap();
    ctx.scope_push();
    ctx.declare_var("T", false, TypeId::Local("U".to_owned()))
        .unwrap();

    assert!(matches!(ctx.ident_kind("T"), IdentKind::Ident { .. }));
    ctx.scope_pop().unwrap();
    assert!(matches!(
        ctx.ident_kind("T"),
        IdentKind::TypeName {
            has_params: false,
            ..
        }
    ));
}

#[test]
fn test_parent_namespace_classifies_members_without_global_state() {
    let mut arena = ValueArena::new();
    let ctx = Context::new(&mut arena);
    ctx.declare_typ("Header", false).unwrap();
    ctx.scope_push();
    ctx.declare_typ("FieldType", true).unwrap();
    let namespace = ctx.scope_pop().unwrap();
    ctx.namespace_set_typ("Header", namespace);
    ctx.declare_var("header", false, TypeId::Local("Header".to_owned()))
        .unwrap();

    ctx.ident_kind("header");
    ctx.namespace_set_parent();
    assert!(matches!(
        ctx.ident_kind("FieldType"),
        IdentKind::TypeName {
            has_params: true,
            ..
        }
    ));
    ctx.namespace_clear_parent();
    assert!(matches!(
        ctx.ident_kind("FieldType"),
        IdentKind::Ident { .. }
    ));
}

#[test]
fn test_contexts_are_isolated() {
    let mut arena = ValueArena::new();
    let ctx_a = Context::new(&mut arena);
    let mut arena_b = ValueArena::new();
    let ctx_b = Context::new(&mut arena_b);
    ctx_a.declare_typ("T", false).unwrap();

    assert!(matches!(ctx_a.ident_kind("T"), IdentKind::TypeName { .. }));
    assert!(matches!(ctx_b.ident_kind("T"), IdentKind::Ident { .. }));
}

#[test]
fn test_go_local_discards_scopes_created_while_locals_are_suspended() {
    let mut arena = ValueArena::new();
    let ctx = Context::new(&mut arena);
    ctx.scope_push();
    ctx.declare_typ("Local", false).unwrap();
    ctx.scope_to_toplevel().unwrap();
    ctx.scope_push();
    ctx.declare_typ("Temporary", false).unwrap();

    ctx.scope_to_local();

    assert!(matches!(
        ctx.ident_kind("Local"),
        IdentKind::TypeName { .. }
    ));
    assert!(matches!(
        ctx.ident_kind("Temporary"),
        IdentKind::Ident { .. }
    ));
}
