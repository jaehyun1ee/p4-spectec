use p4spec_rust::interface::p4::context::{Context, IdentKind, TypeId};

#[test]
fn scopes_shadow_and_restore_identifier_kinds() {
    let context = Context::new();
    context.declare_type("T", false).unwrap();
    context.push_scope();
    context
        .declare_var("T", false, TypeId::Local("U".to_owned()))
        .unwrap();

    assert!(matches!(context.get_kind("T"), IdentKind::Ident { .. }));
    context.pop_scope().unwrap();
    assert!(matches!(
        context.get_kind("T"),
        IdentKind::TypeName {
            has_params: false,
            ..
        }
    ));
}

#[test]
fn parent_namespace_classifies_members_without_global_state() {
    let context = Context::new();
    context.declare_type("Header", false).unwrap();
    context.push_scope();
    context.declare_type("FieldType", true).unwrap();
    let namespace = context.pop_scope().unwrap();
    context.set_type_namespace("Header", namespace);
    context
        .declare_var("header", false, TypeId::Local("Header".to_owned()))
        .unwrap();

    context.get_kind("header");
    context.set_parent_namespace();
    assert!(matches!(
        context.get_kind("FieldType"),
        IdentKind::TypeName {
            has_params: true,
            ..
        }
    ));
    context.clear_parent_namespace();
    assert!(matches!(
        context.get_kind("FieldType"),
        IdentKind::Ident { .. }
    ));
}

#[test]
fn contexts_are_isolated() {
    let context_a = Context::new();
    let context_b = Context::new();
    context_a.declare_type("T", false).unwrap();

    assert!(matches!(
        context_a.get_kind("T"),
        IdentKind::TypeName { .. }
    ));
    assert!(matches!(context_b.get_kind("T"), IdentKind::Ident { .. }));
}
