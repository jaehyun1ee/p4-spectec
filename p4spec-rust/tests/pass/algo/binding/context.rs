use super::super::*;

#[test]
fn test_context_loads_type_and_metavariable_definitions() {
    let extern_id = id("extern_type", 1);
    let defined_id = id("defined_type", 2);
    let variable_id = id("value", 3);
    let bool_typ = crate::phrase! { node: ast::TypKind::Bool, span:  span(2) };
    let def_typ = crate::phrase! { node: ast::DefTypKind::Plain(bool_typ.clone()), span:  span(2) };
    let spec = vec![
        crate::phrase! { node:
        ast::DefKind::ExternTyp(ast::ExternTyp {
            id: extern_id.clone(),
            hints: vec![],
        }), span:
        span(1) },
        crate::phrase! { node:
        ast::DefKind::Typ(ast::TypDef {
            id: defined_id.clone(),
            tparams: vec![],
            def_typ: def_typ.clone(),
            hints: vec![],
        }), span:
        span(2) },
        crate::phrase! { node:
        ast::DefKind::Var(ast::VarDef {
            id: variable_id.clone(),
            typ: bool_typ.clone(),
            hints: vec![],
        }), span:
        span(3) },
    ];

    let mut context = Context::new();
    context.load_spec(&spec);

    assert_eq!(context.tdenv.get(&extern_id), Some(&TypeDef::Extern));
    assert_eq!(
        context.tdenv.get(&defined_id),
        Some(&TypeDef::Defined(vec![], Box::new(def_typ)))
    );
    assert_eq!(context.menv.get(&variable_id), Some(&bool_typ));
    assert!(context.menv.contains_key(&id("bool", 99)));
}
