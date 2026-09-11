use super::super::*;

#[test]
fn test_antiunification_populates_each_path_in_left_to_right_expression_order() {
    let tuple = |bool_l: bool, bool_r: bool, line: i64| {
        exp(
            ast::ExpKind::Tuple(vec![
                exp(ast::ExpKind::Bool(bool_l), ast::TypKind::Bool, line),
                exp(ast::ExpKind::Bool(bool_r), ast::TypKind::Bool, line + 1),
            ]),
            ast::TypKind::Tuple(vec![typ::make::bool(), typ::make::bool()]),
            line,
        )
    };
    let groups = vec![
        vec![tuple(true, false, 1), var_exp("shared", 3)],
        vec![tuple(false, true, 5), var_exp("shared", 7)],
    ];

    let mut ctx = Context::new();
    let (template, prems) =
        antiunify::antiunify(&mut ctx, groups).expect("equivalent tuple inputs");

    assert_eq!(template.len(), 2);
    let ast::ExpKind::Tuple(items) = &template[0].node else {
        panic!("expected tuple template");
    };
    let template_ids = items
        .iter()
        .map(|item| match &item.node {
            ast::ExpKind::Var(id) => id,
            _ => panic!("expected fresh unifier"),
        })
        .collect::<Vec<_>>();
    assert_ne!(template_ids[0].node, template_ids[1].node);
    assert!(ctx.frees.contains(template_ids[0]));
    assert!(ctx.frees.contains(template_ids[1]));
    assert!(matches!(&template[1].node, ast::ExpKind::Var(id) if id.node == "shared"));

    let compared_values = |prems: &[ast::Prem]| {
        prems
            .iter()
            .map(|prem| {
                let ast::PremKind::If(if_prem) = &prem.node else {
                    panic!("expected equality premise");
                };
                let ast::ExpKind::Cmp(_, ast::OpTyp::Bool, _, exp_r) = &if_prem.exp.node else {
                    panic!("expected equality comparison");
                };
                let ast::ExpKind::Bool(value) = exp_r.node else {
                    panic!("expected original boolean expression");
                };
                value
            })
            .collect::<Vec<_>>()
    };
    assert_eq!(prems.len(), 2);
    assert_eq!(compared_values(&prems[0]), vec![true, false]);
    assert_eq!(compared_values(&prems[1]), vec![false, true]);
}

#[test]
fn test_antiunification_freshness_avoids_collisions_within_each_operation() {
    let fresh_unifier = |mut ctx: Context| {
        let (template, _) = antiunify::antiunify(
            &mut ctx,
            vec![
                vec![exp(ast::ExpKind::Bool(true), ast::TypKind::Bool, 1)],
                vec![exp(ast::ExpKind::Bool(false), ast::TypKind::Bool, 2)],
            ],
        )
        .expect("equivalent boolean inputs");
        let ast::ExpKind::Var(id) = &template[0].node else {
            panic!("expected fresh unifier");
        };
        id.clone()
    };

    let id_first = fresh_unifier(Context::new());
    let mut ctx_collision = Context::new();
    ctx_collision.add_free(id_first.clone());
    let id_after_collision = fresh_unifier(ctx_collision);
    let id_independent = fresh_unifier(Context::new());

    assert_ne!(id_after_collision.node, id_first.node);
    assert_eq!(id_independent.node, id_first.node);
}

#[test]
fn test_antiunification_uses_runtime_equivalence_for_plain_type_aliases() {
    let alias_id = id("Flag", 1);
    let alias_typ =
        crate::phrase! { node: ast::TypKind::Var(alias_id.clone(), vec![]), span:  span(1) };
    let mut ctx = Context::new();
    ctx.tdenv.insert(
        alias_id,
        TypeDef::Defined(
            vec![],
            Box::new(
                crate::phrase! { node: ast::DefTypKind::Plain(typ::make::bool()), span:  span(1) },
            ),
        ),
    );
    let alias_value = exp(ast::ExpKind::Bool(true), alias_typ.node, 2);
    let bool_value = exp(ast::ExpKind::Bool(false), ast::TypKind::Bool, 3);

    let (template, prems) =
        antiunify::antiunify(&mut ctx, vec![vec![alias_value], vec![bool_value]])
            .expect("plain alias is equivalent to its underlying type");

    assert!(matches!(template[0].node, ast::ExpKind::Var(_)));
    assert_eq!(prems.iter().map(Vec::len).collect::<Vec<_>>(), vec![1, 1]);
}
