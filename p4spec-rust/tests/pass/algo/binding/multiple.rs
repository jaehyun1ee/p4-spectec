use super::super::*;

#[test]
fn test_multiple_binding_renames_repetitions_and_compares_them_in_occurrence_order() {
    let tuple = exp(
        ast::ExpKind::Tuple(vec![var_exp("x", 1), var_exp("x", 2), var_exp("x", 3)]),
        ast::TypKind::Tuple(vec![typ::bool(), typ::bool(), typ::bool()]),
        1,
    );
    let benv = collect::collect_exp(&Context::new(), &tuple).expect("binding collection");
    let mut context = Context::new();
    let mut renames = multiple::RenameEnv::from_bindings(&benv);

    let renamed = multiple::rename_exp(&mut context, &mut renames, &tuple);
    let side_conditions = multiple::generate_side_conditions(&ICtx::new(), &renames);

    let ast::ExpKind::Tuple(exps) = &renamed.node else {
        panic!("expected tuple binding");
    };
    let ids = exps
        .iter()
        .map(|exp| match &exp.node {
            ast::ExpKind::Var(id) => id,
            _ => panic!("expected variable binding"),
        })
        .collect::<Vec<_>>();
    assert_eq!(ids[0].node, "x");
    assert_ne!(ids[1].node, "x");
    assert_ne!(ids[2].node, "x");
    assert_ne!(ids[1].node, ids[2].node);
    assert_eq!(ids[1].span, span(2));
    assert_eq!(ids[2].span, span(3));

    let [side_condition] = side_conditions.as_slice() else {
        panic!("expected one equality side condition");
    };
    assert_eq!(side_condition.span, span(3));
    let ast_al::PremKind::If(if_prem) = &side_condition.node else {
        panic!("expected conditional premise");
    };
    assert_eq!(if_prem.exp.span, span(3));
    let ast::ExpKind::Bin(_, ast::OpTyp::Bool, first, second) = &if_prem.exp.node else {
        panic!("expected ordered equality conjunction");
    };
    let compared_span = |exp: &ast::Exp| {
        let ast::ExpKind::Cmp(_, ast::OpTyp::Bool, _, exp_r) = &exp.node else {
            panic!("expected equality comparison");
        };
        let ast::ExpKind::Var(id) = &exp_r.node else {
            panic!("expected renamed right operand");
        };
        id.span.clone()
    };
    assert_eq!(compared_span(first), span(2));
    assert_eq!(compared_span(second), span(3));
}

#[test]
fn test_multiple_side_conditions_use_the_rename_environment_dimension() {
    let benv_l = BEnv::singleton(id("x", 1), typ::bool()).add_iter(ast::Iter::List);
    let benv_r = BEnv::singleton(id("x", 2), typ::bool()).add_iter(ast::Iter::List);
    let benv = benv_l.union(benv_r).expect("equivalent dimensions");
    let tuple = exp(
        ast::ExpKind::Tuple(vec![var_exp("x", 1), var_exp("x", 2)]),
        ast::TypKind::Tuple(vec![typ::bool(), typ::bool()]),
        1,
    );
    let mut context = Context::new();
    let mut renames = multiple::RenameEnv::from_bindings(&benv);
    multiple::rename_exp(&mut context, &mut renames, &tuple);

    let premises = multiple::generate_side_conditions(&ICtx::new(), &renames);

    let [premise] = premises.as_slice() else {
        panic!("expected one repeated-binding premise");
    };
    let ast_al::PremKind::Iter(iter_prem) = &premise.node else {
        panic!("expected the collected binding dimension");
    };
    assert_eq!(iter_prem.prem_iter.iter, ast::Iter::List);
    assert!(matches!(iter_prem.prem.node, ast_al::PremKind::If(_)));
}
