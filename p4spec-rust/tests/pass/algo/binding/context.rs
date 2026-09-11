use super::super::*;

#[test]
fn test_context_clone_isolates_local_bindings() {
    let id_free = id("free", 1);
    let ctx = Context::new();
    let mut ctx_local = ctx.clone();

    ctx_local.add_free(id_free.clone());

    assert!(!ctx.frees.contains(&id_free));
    assert!(ctx_local.frees.contains(&id_free));
}

#[test]
fn test_context_loads_type_and_metavariable_definitions() {
    let extern_id = id("extern_type", 1);
    let defined_id = id("defined_type", 2);
    let variable_id = id("value", 3);
    let bool_typ = crate::phrase! { node: ast::TypKind::Bool, span:  span(2) };
    let def_typ = crate::phrase! { node: ast::DefTypKind::Plain(bool_typ.clone()), span:  span(2) };
    let spec = vec![
        crate::phrase! { node:
        ast::DefKind::Typ(ast::TypDef::Extern(ast::ExternTyp {
            id: extern_id.clone(),
            hints: vec![],
        })), span:
        span(1) },
        crate::phrase! { node:
        ast::DefKind::Typ(ast::TypDef::Defined(Box::new(ast::DefinedTyp {
            id: defined_id.clone(),
            tparams: vec![],
            def_typ: def_typ.clone(),
            hints: vec![],
        }))), span:
        span(2) },
        crate::phrase! { node:
        ast::DefKind::Var(ast::VarDef {
            id: variable_id.clone(),
            typ: bool_typ.clone(),
            hints: vec![],
        }), span:
        span(3) },
    ];

    let mut ctx = Context::new();
    ctx.load_spec(&spec);

    assert_eq!(ctx.tdenv.get(&extern_id), Some(&TypeDef::Extern));
    assert_eq!(
        ctx.tdenv.get(&defined_id),
        Some(&TypeDef::Defined(vec![], Box::new(def_typ)))
    );
    assert_eq!(ctx.menv.get(&variable_id), Some(&bool_typ));
    assert!(ctx.menv.contains_key(&id("bool", 99)));
}
