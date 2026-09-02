use super::super::*;

#[test]
fn test_conversion_preserves_binding_match_and_cast_guards_before_bindings() {
    let parent_id = id("Parent", 1);
    let child_id = id("Child", 2);
    let parent_typ =
        crate::phrase! { node: ast::TypKind::Var(parent_id.clone(), vec![]), span:  span(1) };
    let child_typ =
        crate::phrase! { node: ast::TypKind::Var(child_id.clone(), vec![]), span:  span(2) };
    let parent_origin = crate::phrase! { node: (parent_id.clone(), vec![]), span:  span(1) };
    let child_origin = crate::phrase! { node: (child_id.clone(), vec![]), span:  span(2) };
    let parent_def = crate::phrase! { node:
    ast::DefKind::Typ(ast::TypDef {
        id: parent_id,
        tparams: vec![],
        def_typ: crate::phrase! { node:
            ast::DefTypKind::Variant(vec![
                (not_typ("A", 1), parent_origin.clone(), vec![]),
                (not_typ("B", 1), parent_origin, vec![]),
            ]), span:
            span(1) },
        hints: vec![],
    }), span:
    span(1) };
    let child_def = crate::phrase! { node:
    ast::DefKind::Typ(ast::TypDef {
        id: child_id,
        tparams: vec![],
        def_typ: crate::phrase! { node:
            ast::DefTypKind::Variant(vec![(not_typ("A", 2), child_origin, vec![])]), span:
            span(2) },
        hints: vec![],
    }), span:
    span(2) };
    let typ_bool = typ::bool();
    let typ_list = typ::list(typ_bool.clone());
    let exp_list = exp(
        ast::ExpKind::List(vec![typed_var_exp("item", &typ_bool, 10)]),
        typ_list.node.clone(),
        10,
    );
    let exp_upcast = exp(
        ast::ExpKind::UpCast(
            parent_typ.clone(),
            Box::new(typed_var_exp("child", &child_typ, 11)),
        ),
        parent_typ.node.clone(),
        11,
    );
    let exp_output = exp(
        ast::ExpKind::Tuple(vec![
            typed_var_exp("item", &typ_bool, 12),
            typed_var_exp("child", &child_typ, 12),
        ]),
        ast::TypKind::Tuple(vec![typ_bool.clone(), child_typ.clone()]),
        12,
    );
    let mut spec = function_spec(
        vec![typ_list, parent_typ],
        vec![exp_list, exp_upcast],
        exp_output,
        vec![],
    );
    spec.insert(0, child_def);
    spec.insert(0, parent_def);

    let converted = algo::convert(&spec).expect("partial binding conversion");
    let crate::lang::al::ast::DefKind::FuncDec(function) = &converted[2].node else {
        panic!("expected converted function");
    };
    let [match_guard, list_binding, subtype_guard, cast_binding] =
        function.clauses[0].node.premises.as_slice()
    else {
        panic!("expected match/bind and subtype/downcast pairs");
    };

    assert!(matches!(
        &match_guard.node,
        ast_al::PremKind::If(ast_al::IfPrem {
            exp: NotePhrase {
                node: ast::ExpKind::Match(_, ast::Pattern::List(ast::ListPattern::Fixed(1))),
                ..
            }
        })
    ));
    assert!(matches!(&list_binding.node, ast_al::PremKind::Let(_)));
    assert!(matches!(
        &subtype_guard.node,
        ast_al::PremKind::If(ast_al::IfPrem {
            exp: NotePhrase {
                node: ast::ExpKind::Sub(_, typ, _),
                ..
            }
        }) if typ.syntax_eq(&child_typ)
    ));
    assert!(matches!(
        &cast_binding.node,
        ast_al::PremKind::Let(ast_al::LetPrem {
            exp_r: NotePhrase {
                node: ast::ExpKind::DownCast(typ, _),
                ..
            },
            ..
        }) if typ.syntax_eq(&child_typ)
    ));
}

#[test]
fn test_partial_binding_preserves_expression_and_premise_iteration_dimensions() {
    let bool_value = exp(ast::ExpKind::Bool(true), ast::TypKind::Bool, 2);
    let tuple = exp(
        ast::ExpKind::Tuple(vec![var_exp("x", 1), bool_value]),
        ast::TypKind::Tuple(vec![typ::bool(), typ::bool()]),
        1,
    );
    let iterated = exp(
        ast::ExpKind::Iter(
            Box::new(tuple),
            (
                ast::Iter::List,
                vec![ast::Var {
                    id: id("x", 1),
                    typ: typ::bool(),
                    iters: vec![],
                }],
            ),
        ),
        ast::TypKind::Iter(
            Box::new(crate::phrase! { node:
            ast::TypKind::Tuple(vec![typ::bool(), typ::bool()]), span:
            span(1) }),
            ast::Iter::List,
        ),
        1,
    );
    let mut context = Context::new();
    let benv = collect::collect_exp(&context, &iterated).expect("binding collection");
    let ids_bind = benv_domain(&benv);
    let mut renames = partial::RenameEnv::new();

    let (_, renamed) = partial::rename_exp(
        &mut context,
        &ids_bind,
        &mut renames,
        IterationContext::new(),
        &iterated,
    )
    .expect("partial binding rename");
    let premises = partial::generate_prems(&context, &IterationContext::new(), &renames)
        .expect("partial binding premises");

    let ast::ExpKind::Iter(exp_inner, (ast::Iter::List, vars)) = &renamed.node else {
        panic!("expected iterated binding");
    };
    let ast::ExpKind::Tuple(exps) = &exp_inner.node else {
        panic!("expected tuple binding");
    };
    let ast::ExpKind::Var(id_rename) = &exps[1].node else {
        panic!("expected bound value to be renamed");
    };
    assert_eq!(vars.len(), 2);
    assert_eq!(vars[0].id.node, "x");
    assert_eq!(vars[1].id, *id_rename);

    let [premise] = premises.as_slice() else {
        panic!("expected one equality premise");
    };
    let ast_al::PremKind::Iter(iterated_prem) = &premise.node else {
        panic!("expected premise iteration");
    };
    assert_eq!(iterated_prem.iter_prem.iter, ast::Iter::List);
    assert_eq!(iterated_prem.iter_prem.vars_bound.len(), 1);
    assert_eq!(iterated_prem.iter_prem.vars_bound[0].id, *id_rename);
    let ast_al::PremKind::If(if_prem) = &iterated_prem.prem.node else {
        panic!("expected equality side condition");
    };
    let ast::ExpKind::Cmp(_, ast::OpTyp::Bool, exp_l, exp_r) = &if_prem.exp.node else {
        panic!("expected equality comparison");
    };
    assert!(matches!(&exp_l.node, ast::ExpKind::Var(id) if id == id_rename));
    assert!(matches!(exp_r.node, ast::ExpKind::Bool(true)));
}

#[test]
fn test_partial_binding_preserves_nested_iteration_order_and_dimensions() {
    let tuple_typ = crate::phrase! { node: ast::TypKind::Tuple(vec![typ::bool(), typ::bool()]), span:  span(1) };
    let tuple = exp(
        ast::ExpKind::Tuple(vec![
            var_exp("x", 1),
            exp(ast::ExpKind::Bool(true), ast::TypKind::Bool, 2),
        ]),
        tuple_typ.node.clone(),
        1,
    );
    let inner_typ = crate::phrase! { node:
    ast::TypKind::Iter(Box::new(tuple_typ), ast::Iter::Opt), span:
    span(1) };
    let inner = exp(
        ast::ExpKind::Iter(
            Box::new(tuple),
            (
                ast::Iter::Opt,
                vec![ast::Var {
                    id: id("x", 1),
                    typ: typ::bool(),
                    iters: vec![],
                }],
            ),
        ),
        inner_typ.node.clone(),
        1,
    );
    let iterated = exp(
        ast::ExpKind::Iter(
            Box::new(inner),
            (
                ast::Iter::List,
                vec![ast::Var {
                    id: id("x", 1),
                    typ: typ::bool(),
                    iters: vec![ast::Iter::Opt],
                }],
            ),
        ),
        ast::TypKind::Iter(Box::new(inner_typ), ast::Iter::List),
        1,
    );
    let mut context = Context::new();
    let benv = collect::collect_exp(&context, &iterated).expect("binding collection");
    let ids_bind = benv_domain(&benv);
    let mut renames = partial::RenameEnv::new();

    let (_, renamed) = partial::rename_exp(
        &mut context,
        &ids_bind,
        &mut renames,
        IterationContext::new(),
        &iterated,
    )
    .expect("nested partial binding rename");
    let premises = partial::generate_prems(&context, &IterationContext::new(), &renames)
        .expect("nested partial binding premises");

    let ast::ExpKind::Iter(inner, (ast::Iter::List, outer_vars)) = &renamed.node else {
        panic!("expected outer list iteration");
    };
    let ast::ExpKind::Iter(tuple, (ast::Iter::Opt, inner_vars)) = &inner.node else {
        panic!("expected inner optional iteration");
    };
    let ast::ExpKind::Tuple(exps) = &tuple.node else {
        panic!("expected iterated tuple");
    };
    let ast::ExpKind::Var(id_rename) = &exps[1].node else {
        panic!("expected nested bound value rename");
    };
    assert_eq!(inner_vars.len(), 2);
    assert_eq!(outer_vars.len(), 2);
    assert_eq!(inner_vars[1].id, *id_rename);
    assert_eq!(outer_vars[1].id, *id_rename);

    let [premise] = premises.as_slice() else {
        panic!("expected one nested equality premise");
    };
    let ast_al::PremKind::Iter(outer) = &premise.node else {
        panic!("expected outer premise iteration");
    };
    assert_eq!(outer.iter_prem.iter, ast::Iter::List);
    assert_eq!(outer.iter_prem.vars_bound[0].id, *id_rename);
    let ast_al::PremKind::Iter(inner) = &outer.prem.node else {
        panic!("expected inner premise iteration");
    };
    assert_eq!(inner.iter_prem.iter, ast::Iter::Opt);
    assert_eq!(inner.iter_prem.vars_bound[0].id, *id_rename);
}

#[test]
fn test_partial_binding_rolls_back_context_and_renames_after_late_failure() {
    let initial = exp(ast::ExpKind::Bool(true), ast::TypKind::Bool, 1);
    let mut context = Context::new();
    let benv_initial = collect::collect_exp(&context, &initial).expect("binding collection");
    let ids_bind_initial = benv_domain(&benv_initial);
    let mut renames = partial::RenameEnv::new();
    partial::rename_exp(
        &mut context,
        &ids_bind_initial,
        &mut renames,
        IterationContext::new(),
        &initial,
    )
    .expect("initial partial binding rename");
    let frees_before = context.frees().clone();
    let premise_count_before =
        partial::generate_prems(&context, &IterationContext::new(), &renames)
            .expect("initial premises")
            .len();

    let missing_typ = ast::TypKind::Var(id("Missing", 12), vec![]);
    let case = exp(
        ast::ExpKind::Case(Box::new(Mixfix::Arg(var_exp("y", 12)))),
        missing_typ.clone(),
        12,
    );
    let tuple = exp(
        ast::ExpKind::Tuple(vec![
            exp(ast::ExpKind::Bool(false), ast::TypKind::Bool, 11),
            case,
        ]),
        ast::TypKind::Tuple(vec![
            typ::bool(),
            crate::phrase! { node: missing_typ, span:  span(12) },
        ]),
        11,
    );
    let benv = collect::collect_exp(&context, &tuple).expect("binding collection");
    let ids_bind = benv_domain(&benv);

    let error = partial::rename_exp(
        &mut context,
        &ids_bind,
        &mut renames,
        IterationContext::new(),
        &tuple,
    )
    .expect_err("undefined case type");

    assert_eq!(error.kind, AlgoErrorKind::UndefinedType);
    assert_eq!(error.span, span(12));
    assert_eq!(context.frees(), &frees_before);
    assert_eq!(
        partial::generate_prems(&context, &IterationContext::new(), &renames)
            .expect("rolled-back premises")
            .len(),
        premise_count_before
    );
}

#[test]
fn test_partial_case_and_list_bindings_generate_match_then_bind_premises_in_source_order() {
    let choice_id = id("Choice", 1);
    let choice_typ =
        crate::phrase! { node: ast::TypKind::Var(choice_id.clone(), vec![]), span:  span(1) };
    let origin = crate::phrase! { node: (choice_id.clone(), vec![]), span:  span(1) };
    let def_typ = crate::phrase! { node:
    ast::DefTypKind::Variant(vec![
        (not_typ("A", 1), origin.clone(), vec![]),
        (not_typ("B", 1), origin, vec![]),
    ]), span:
    span(1) };
    let mut context = Context::new();
    context
        .tdenv_mut()
        .insert(choice_id, TypeDef::Defined(vec![], Box::new(def_typ)));

    let keyword = crate::phrase! { node: Atom::Keyword("A".to_owned()), span:  span(2) };
    let case = exp(
        ast::ExpKind::Case(Box::new(Mixfix::Seq(vec![
            Mixfix::Atom(keyword),
            Mixfix::Arg(var_exp("y", 2)),
        ]))),
        choice_typ.node.clone(),
        2,
    );
    let list_typ = crate::phrase! { node:
    ast::TypKind::Iter(Box::new(typ::bool()), ast::Iter::List), span:
    span(3) };
    let list = exp(
        ast::ExpKind::List(vec![var_exp("z", 3)]),
        list_typ.node.clone(),
        3,
    );
    let tuple = exp(
        ast::ExpKind::Tuple(vec![case, list]),
        ast::TypKind::Tuple(vec![choice_typ, list_typ]),
        2,
    );
    let benv = collect::collect_exp(&context, &tuple).expect("binding collection");
    let ids_bind = benv_domain(&benv);
    let mut renames = partial::RenameEnv::new();

    let (_, renamed) = partial::rename_exp(
        &mut context,
        &ids_bind,
        &mut renames,
        IterationContext::new(),
        &tuple,
    )
    .expect("partial binding rename");
    let premises = partial::generate_prems(&context, &IterationContext::new(), &renames)
        .expect("partial binding premises");

    let ast::ExpKind::Tuple(exps) = &renamed.node else {
        panic!("expected tuple binding");
    };
    assert!(matches!(exps[0].node, ast::ExpKind::Var(_)));
    assert!(matches!(exps[1].node, ast::ExpKind::Iter(_, _)));
    assert_eq!(premises.len(), 4);
    assert!(matches!(
        &premises[0].node,
        ast_al::PremKind::If(ast_al::IfPrem {
            exp: NotePhrase {
                node: ast::ExpKind::Match(_, ast::Pattern::Case(_)),
                ..
            }
        })
    ));
    assert!(matches!(
        &premises[1].node,
        ast_al::PremKind::Let(ast_al::LetPrem {
            exp_l: NotePhrase {
                node: ast::ExpKind::Case(_),
                ..
            },
            ..
        })
    ));
    assert!(matches!(
        &premises[2].node,
        ast_al::PremKind::If(ast_al::IfPrem {
            exp: NotePhrase {
                node: ast::ExpKind::Match(_, ast::Pattern::List(ast::ListPattern::Fixed(1))),
                ..
            }
        })
    ));
    assert!(matches!(
        &premises[3].node,
        ast_al::PremKind::Let(ast_al::LetPrem {
            exp_l: NotePhrase {
                node: ast::ExpKind::List(_),
                ..
            },
            ..
        })
    ));
}

#[test]
fn test_partial_upcast_binding_checks_subtype_before_binding_the_downcast_value() {
    let parent_id = id("Parent", 1);
    let child_id = id("Child", 1);
    let parent_typ =
        crate::phrase! { node: ast::TypKind::Var(parent_id.clone(), vec![]), span:  span(1) };
    let child_typ =
        crate::phrase! { node: ast::TypKind::Var(child_id.clone(), vec![]), span:  span(1) };
    let parent_origin = crate::phrase! { node: (parent_id.clone(), vec![]), span:  span(1) };
    let child_origin = crate::phrase! { node: (child_id.clone(), vec![]), span:  span(1) };
    let mut context = Context::new();
    context.tdenv_mut().insert(
        parent_id,
        TypeDef::Defined(
            vec![],
            Box::new(crate::phrase! { node:
            ast::DefTypKind::Variant(vec![
                (not_typ("A", 1), parent_origin.clone(), vec![]),
                (not_typ("B", 1), parent_origin, vec![]),
            ]), span:
            span(1) }),
        ),
    );
    context.tdenv_mut().insert(
        child_id,
        TypeDef::Defined(
            vec![],
            Box::new(crate::phrase! { node:
            ast::DefTypKind::Variant(vec![(not_typ("A", 1), child_origin, vec![])]), span:
            span(1) }),
        ),
    );
    let child_var = exp(ast::ExpKind::Var(id("child", 2)), child_typ.node.clone(), 2);
    let upcast = exp(
        ast::ExpKind::UpCast(parent_typ.clone(), Box::new(child_var)),
        parent_typ.node.clone(),
        2,
    );
    let benv = collect::collect_exp(&context, &upcast).expect("binding collection");
    let ids_bind = benv_domain(&benv);
    let mut renames = partial::RenameEnv::new();

    partial::rename_exp(
        &mut context,
        &ids_bind,
        &mut renames,
        IterationContext::new(),
        &upcast,
    )
    .expect("partial binding rename");
    let premises = partial::generate_prems(&context, &IterationContext::new(), &renames)
        .expect("partial binding premises");

    let [subtype, binding] = premises.as_slice() else {
        panic!("expected subtype and binding premises");
    };
    assert!(matches!(
        &subtype.node,
        ast_al::PremKind::If(ast_al::IfPrem {
            exp: NotePhrase {
                node: ast::ExpKind::Sub(_, typ, _),
                ..
            }
        }) if typ.syntax_eq(&child_typ)
    ));
    assert!(matches!(
        &binding.node,
        ast_al::PremKind::Let(ast_al::LetPrem {
            exp_l: NotePhrase {
                node: ast::ExpKind::Var(_),
                ..
            },
            exp_r: NotePhrase {
                node: ast::ExpKind::DownCast(typ, _),
                ..
            }
        }) if typ.syntax_eq(&child_typ)
    ));
}
