use super::super::*;

#[test]
fn test_conversion_inserts_index_guards_at_evaluation_sites_in_source_order() {
    fn assert_index_guard(
        prem: &ast::Prem,
        guard_span: Span,
        base_span: Span,
        index_name: &str,
        index_span: Span,
    ) {
        assert_eq!(prem.span, guard_span);
        let ast::PremKind::If(if_prem) = &prem.node else {
            panic!("expected index guard premise");
        };
        assert_eq!(if_prem.exp.span, guard_span);
        let ast::ExpKind::Cmp(
            ast::CmpOp::Num(xl::num::CmpOp::Lt),
            ast::OpTyp::Bool,
            exp_i,
            exp_len,
        ) = &if_prem.exp.node
        else {
            panic!("expected strict index bound");
        };
        assert_eq!(exp_i.span, index_span);
        assert!(matches!(&exp_i.node, ast::ExpKind::Var(id) if id.node == index_name));
        assert_eq!(exp_len.span, guard_span);
        let ast::ExpKind::Len(exp_base) = &exp_len.node else {
            panic!("expected indexed-base length");
        };
        assert_eq!(exp_base.span, base_span);
    }

    let typ_bool = typ::bool();
    let typ_nat = typ::nat();
    let typ_list = typ::list(typ_bool.clone());
    let exp_index_prem = exp(
        ast::ExpKind::Idx(
            Box::new(iterated_var_exp("xs", &typ_bool, ast::Iter::List, 10)),
            Box::new(typed_var_exp("i", &typ_nat, 11)),
        ),
        ast::TypKind::Bool,
        12,
    );
    let exp_condition = exp(
        ast::ExpKind::Cmp(
            ast::CmpOp::Bool(xl::bool::CmpOp::Eq),
            ast::OpTyp::Bool,
            Box::new(exp_index_prem),
            Box::new(exp(ast::ExpKind::Bool(true), ast::TypKind::Bool, 13)),
        ),
        ast::TypKind::Bool,
        13,
    );
    let prem_source = if_prem(exp_condition);
    let exp_output = exp(
        ast::ExpKind::Idx(
            Box::new(iterated_var_exp("xs", &typ_bool, ast::Iter::List, 20)),
            Box::new(typed_var_exp("j", &typ_nat, 21)),
        ),
        ast::TypKind::Bool,
        22,
    );
    let spec = function_spec(
        vec![typ_list, typ_nat.clone(), typ_nat.clone()],
        vec![
            iterated_var_exp("xs", &typ_bool, ast::Iter::List, 2),
            typed_var_exp("i", &typ_nat, 3),
            typed_var_exp("j", &typ_nat, 4),
        ],
        exp_output.clone(),
        vec![prem_source.clone()],
    );

    let converted = algo::convert(&spec).expect("guarded conversion");
    let clause = function_clause(&converted);
    let [guard_premise, source_premise, guard_output] = clause.node.premises.as_slice() else {
        panic!("expected premise and output index guards");
    };

    assert_index_guard(guard_premise, span(12), span(10), "i", span(11));
    assert_eq!(source_premise, &prem_source);
    assert_index_guard(guard_output, span(22), span(20), "j", span(21));
    assert_eq!(clause.node.expression, exp_output);
    assert_eq!(clause.span, span(1));
}

#[test]
fn test_conversion_inserts_list_and_optional_iteration_guards_in_source_order() {
    fn dimension_name(exp: &ast::Exp, iter: ast::Iter) -> &str {
        let ast::ExpKind::Iter(exp_inner, (actual_iter, _)) = &exp.node else {
            panic!("expected dimension expression");
        };
        assert_eq!(*actual_iter, iter);
        let ast::ExpKind::Var(id) = &exp_inner.node else {
            panic!("expected dimension variable");
        };
        &id.node
    }

    fn list_pair(exp: &ast::Exp) -> (&str, &str) {
        let ast::ExpKind::Cmp(
            ast::CmpOp::Bool(xl::bool::CmpOp::Eq),
            ast::OpTyp::Bool,
            exp_l,
            exp_r,
        ) = &exp.node
        else {
            panic!("expected list-length equality");
        };
        let ast::ExpKind::Len(exp_l) = &exp_l.node else {
            panic!("expected left length");
        };
        let ast::ExpKind::Len(exp_r) = &exp_r.node else {
            panic!("expected right length");
        };
        (
            dimension_name(exp_l, ast::Iter::List),
            dimension_name(exp_r, ast::Iter::List),
        )
    }

    fn optional_name(exp: &ast::Exp) -> &str {
        let ast::ExpKind::Cmp(
            ast::CmpOp::Bool(xl::bool::CmpOp::Eq),
            ast::OpTyp::Bool,
            exp_l,
            exp_r,
        ) = &exp.node
        else {
            panic!("expected optional-presence equality");
        };
        assert!(matches!(exp_r.node, ast::ExpKind::Opt(None)));
        dimension_name(exp_l, ast::Iter::Opt)
    }

    fn optional_pair(exp: &ast::Exp) -> (&str, &str) {
        let ast::ExpKind::Bin(
            ast::BinOp::Bool(xl::bool::BinOp::Equiv),
            ast::OpTyp::Bool,
            exp_l,
            exp_r,
        ) = &exp.node
        else {
            panic!("expected optional-presence equivalence");
        };
        (optional_name(exp_l), optional_name(exp_r))
    }

    let typ_bool = typ::bool();
    let list_names = [("x", 2), ("y", 3), ("z", 4)];
    let optional_names = [("p", 5), ("q", 6), ("r", 7)];
    let mut params = vec![typ::list(typ_bool.clone()); list_names.len()];
    params.extend(vec![typ::opt(typ_bool.clone()); optional_names.len()]);
    let mut args = list_names
        .iter()
        .map(|(name, line)| iterated_var_exp(name, &typ_bool, ast::Iter::List, *line))
        .collect::<Vec<_>>();
    args.extend(
        optional_names
            .iter()
            .map(|(name, line)| iterated_var_exp(name, &typ_bool, ast::Iter::Opt, *line)),
    );
    let exp_list = joint_iteration(&list_names, ast::Iter::List, 20);
    let exp_optional = joint_iteration(&optional_names, ast::Iter::Opt, 21);
    let typ_output = ast::TypKind::Tuple(vec![
        crate::phrase! { node: exp_list.note.clone(), span:  exp_list.span.clone() },
        crate::phrase! { node: exp_optional.note.clone(), span:  exp_optional.span.clone() },
    ]);
    let exp_output = exp(
        ast::ExpKind::Tuple(vec![exp_list, exp_optional]),
        typ_output,
        22,
    );
    let spec = function_spec(params, args, exp_output, vec![]);

    let converted = algo::convert(&spec).expect("guarded joint iterations");
    let clause = function_clause(&converted);
    let [prem_list, prem_optional] = clause.node.premises.as_slice() else {
        panic!("expected list and optional guards");
    };

    assert_eq!(prem_list.span, Span::over(&[span(2), span(3), span(4)]));
    let ast::PremKind::If(if_list) = &prem_list.node else {
        panic!("expected list guard premise");
    };
    let ast::ExpKind::Bin(
        ast::BinOp::Bool(xl::bool::BinOp::And),
        ast::OpTyp::Bool,
        pair_xy,
        pair_yz,
    ) = &if_list.exp.node
    else {
        panic!("expected pairwise list guard conjunction");
    };
    assert_eq!(list_pair(pair_xy), ("x", "y"));
    assert_eq!(list_pair(pair_yz), ("y", "z"));

    assert_eq!(prem_optional.span, Span::over(&[span(5), span(6), span(7)]));
    let ast::PremKind::If(if_optional) = &prem_optional.node else {
        panic!("expected optional guard premise");
    };
    let ast::ExpKind::Bin(
        ast::BinOp::Bool(xl::bool::BinOp::And),
        ast::OpTyp::Bool,
        pair_pq,
        pair_qr,
    ) = &if_optional.exp.node
    else {
        panic!("expected pairwise optional guard conjunction");
    };
    assert_eq!(optional_pair(pair_pq), ("p", "q"));
    assert_eq!(optional_pair(pair_qr), ("q", "r"));
}

#[test]
fn test_conversion_omits_iteration_guards_entailed_by_prior_premises() {
    let typ_bool = typ::bool();
    let names = [("x", 2), ("y", 3), ("z", 4)];
    let args = names
        .iter()
        .map(|(name, line)| iterated_var_exp(name, &typ_bool, ast::Iter::List, *line))
        .collect::<Vec<_>>();
    let prem_xy = equality_prem(len_exp("x", 10), len_exp("y", 11), 12);
    let prem_yz = equality_prem(len_exp("y", 13), len_exp("z", 14), 15);
    let exp_output = joint_iteration(&[("x", 20), ("z", 21)], ast::Iter::List, 22);
    let spec = function_spec(
        vec![typ::list(typ_bool); names.len()],
        args,
        exp_output,
        vec![prem_xy.clone(), prem_yz.clone()],
    );

    let converted = algo::convert(&spec).expect("transitively guarded iteration");
    let premises = &function_clause(&converted).node.premises;

    assert_eq!(premises, &[prem_xy, prem_yz]);
}

#[test]
fn test_conversion_preserves_numeric_and_slice_checks_before_output_guards() {
    let typ_nat = typ::nat();
    let typ_list = typ::list(typ_nat.clone());
    let exp_zero = exp(
        ast::ExpKind::Num(ast::Num::Nat(0_u64.into())),
        ast::TypKind::Num(xl::num::Typ::Nat),
        10,
    );
    let exp_nonzero = exp(
        ast::ExpKind::Cmp(
            ast::CmpOp::Bool(xl::bool::CmpOp::Ne),
            ast::OpTyp::Bool,
            Box::new(typed_var_exp("d", &typ_nat, 10)),
            Box::new(exp_zero),
        ),
        ast::TypKind::Bool,
        10,
    );
    let prem_nonzero = if_prem(exp_nonzero);
    let exp_end = exp(
        ast::ExpKind::Bin(
            ast::BinOp::Num(xl::num::BinOp::Add),
            ast::OpTyp::Nat,
            Box::new(typed_var_exp("offset", &typ_nat, 11)),
            Box::new(typed_var_exp("length", &typ_nat, 11)),
        ),
        ast::TypKind::Num(xl::num::Typ::Nat),
        11,
    );
    let exp_base_for_len = iterated_var_exp("xs", &typ_nat, ast::Iter::List, 12);
    let exp_length = exp(
        ast::ExpKind::Len(Box::new(exp_base_for_len)),
        ast::TypKind::Num(xl::num::Typ::Nat),
        12,
    );
    let exp_slice_bound = exp(
        ast::ExpKind::Cmp(
            ast::CmpOp::Num(xl::num::CmpOp::Le),
            ast::OpTyp::Bool,
            Box::new(exp_end),
            Box::new(exp_length),
        ),
        ast::TypKind::Bool,
        12,
    );
    let prem_slice_bound = if_prem(exp_slice_bound);
    let exp_index = exp(
        ast::ExpKind::Idx(
            Box::new(iterated_var_exp("xs", &typ_nat, ast::Iter::List, 20)),
            Box::new(typed_var_exp("index", &typ_nat, 21)),
        ),
        ast::TypKind::Num(xl::num::Typ::Nat),
        22,
    );
    let exp_division = exp(
        ast::ExpKind::Bin(
            ast::BinOp::Num(xl::num::BinOp::Div),
            ast::OpTyp::Nat,
            Box::new(exp_index),
            Box::new(typed_var_exp("d", &typ_nat, 23)),
        ),
        ast::TypKind::Num(xl::num::Typ::Nat),
        23,
    );
    let exp_slice = exp(
        ast::ExpKind::Slice(
            Box::new(iterated_var_exp("xs", &typ_nat, ast::Iter::List, 24)),
            Box::new(typed_var_exp("offset", &typ_nat, 24)),
            Box::new(typed_var_exp("length", &typ_nat, 24)),
        ),
        typ_list.node.clone(),
        24,
    );
    let exp_remainder = exp(
        ast::ExpKind::Bin(
            ast::BinOp::Num(xl::num::BinOp::Mod),
            ast::OpTyp::Nat,
            Box::new(typed_var_exp("value", &typ_nat, 25)),
            Box::new(typed_var_exp("d", &typ_nat, 25)),
        ),
        ast::TypKind::Num(xl::num::Typ::Nat),
        25,
    );
    let exp_output = exp(
        ast::ExpKind::Tuple(vec![exp_division, exp_slice, exp_remainder]),
        ast::TypKind::Tuple(vec![typ_nat.clone(), typ_list.clone(), typ_nat.clone()]),
        26,
    );
    let params = vec![
        typ_list,
        typ_nat.clone(),
        typ_nat.clone(),
        typ_nat.clone(),
        typ_nat.clone(),
        typ_nat.clone(),
    ];
    let args = vec![
        iterated_var_exp("xs", &typ_nat, ast::Iter::List, 2),
        typed_var_exp("index", &typ_nat, 3),
        typed_var_exp("offset", &typ_nat, 4),
        typed_var_exp("length", &typ_nat, 5),
        typed_var_exp("value", &typ_nat, 6),
        typed_var_exp("d", &typ_nat, 7),
    ];
    let spec = function_spec(
        params,
        args,
        exp_output,
        vec![prem_nonzero.clone(), prem_slice_bound.clone()],
    );

    let converted = algo::convert(&spec).expect("numeric and slice conversion");
    let premises = &function_clause(&converted).node.premises;
    let [actual_nonzero, actual_slice_bound, index_guard] = premises.as_slice() else {
        panic!("expected two explicit checks and one index guard");
    };

    assert_eq!(actual_nonzero, &prem_nonzero);
    assert_eq!(actual_slice_bound, &prem_slice_bound);
    let ast::PremKind::If(if_index) = &index_guard.node else {
        panic!("expected index guard");
    };
    assert!(matches!(
        if_index.exp.node,
        ast::ExpKind::Cmp(ast::CmpOp::Num(xl::num::CmpOp::Lt), _, _, _)
    ));
    assert_eq!(index_guard.span, span(22));
}

#[test]
fn test_conversion_distinguishes_let_must_guards_from_insert_guards() {
    let typ_bool = typ::bool();
    let exp_l = joint_iteration(&[("bound_l", 20), ("bound_r", 21)], ast::Iter::List, 22);
    let exp_r = joint_iteration(&[("input_l", 23), ("input_r", 24)], ast::Iter::List, 25);
    let prem_equality = equality_prem(exp_l, exp_r, 30);
    let exp_output = joint_iteration(&[("bound_l", 20), ("bound_r", 21)], ast::Iter::List, 40);
    let spec = function_spec(
        vec![typ::list(typ_bool.clone()), typ::list(typ_bool.clone())],
        vec![
            iterated_var_exp("input_l", &typ_bool, ast::Iter::List, 2),
            iterated_var_exp("input_r", &typ_bool, ast::Iter::List, 3),
        ],
        exp_output.clone(),
        vec![prem_equality],
    );

    let converted = algo::convert(&spec).expect("let guard conversion");
    let clause = function_clause(&converted);
    let [right_guard, let_premise] = clause.node.premises.as_slice() else {
        panic!("expected only the right guard before the generated let premise");
    };

    assert_eq!(right_guard.span, Span::over(&[span(23), span(24)]));
    assert!(matches!(right_guard.node, ast::PremKind::If(_)));
    assert_eq!(let_premise.span, span(30));
    let ast::PremKind::Let(let_prem) = &let_premise.node else {
        panic!("expected binding analysis to produce a let premise");
    };
    assert!(matches!(let_prem.exp_l.node, ast::ExpKind::Iter(_, _)));
    assert!(matches!(let_prem.exp_r.node, ast::ExpKind::Iter(_, _)));
    assert_eq!(clause.node.expression, exp_output);
}

#[test]
fn test_conversion_distinguishes_iterated_must_guards_from_insert_guards() {
    let typ_bool = typ::bool();
    let exp_condition = exp(
        ast::ExpKind::Cmp(
            ast::CmpOp::Bool(xl::bool::CmpOp::Eq),
            ast::OpTyp::Bool,
            Box::new(typed_var_exp("left", &typ_bool, 12)),
            Box::new(typed_var_exp("right", &typ_bool, 13)),
        ),
        ast::TypKind::Bool,
        14,
    );
    let prem_iterated = crate::phrase! { node:
    ast::PremKind::Iter(ast::IteratedPrem {
        prem: Box::new(if_prem(exp_condition)),
        iter_prem: ast::IterPrem {
            iter: ast::Iter::List,
            vars_bound: vec![
                iteration_var("left", typ_bool.clone(), 10),
                iteration_var("right", typ_bool.clone(), 11),
            ],
            vars_bind: vec![],
        },
    }), span:
    span(15) };
    let insert_spec = function_spec(
        vec![typ::list(typ_bool.clone()), typ::list(typ_bool.clone())],
        vec![
            iterated_var_exp("left", &typ_bool, ast::Iter::List, 2),
            iterated_var_exp("right", &typ_bool, ast::Iter::List, 3),
        ],
        exp(ast::ExpKind::Bool(true), ast::TypKind::Bool, 16),
        vec![prem_iterated],
    );

    let converted = algo::convert(&insert_spec).expect("iterated insertion conversion");
    let [joint_guard, source_premise] = function_clause(&converted).node.premises.as_slice() else {
        panic!("expected a joint guard before the iterated premise");
    };
    assert_eq!(joint_guard.span, Span::over(&[span(10), span(11)]));
    assert!(matches!(joint_guard.node, ast::PremKind::If(_)));
    assert_eq!(source_premise.span, span(14));
    assert!(matches!(source_premise.node, ast::PremKind::Iter(_)));

    let prem_binding = crate::phrase! { node:
    ast::PremKind::Iter(ast::IteratedPrem {
        prem: Box::new(equality_prem(
            typed_var_exp("output", &typ_bool, 23),
            typed_var_exp("input", &typ_bool, 22),
            24,
        )),
        iter_prem: ast::IterPrem {
            iter: ast::Iter::List,
            vars_bound: vec![iteration_var("input", typ_bool.clone(), 22)],
            vars_bind: vec![],
        },
    }), span:
    span(25) };
    let exp_output = joint_iteration(&[("input", 22), ("output", 23)], ast::Iter::List, 30);
    let must_spec = function_spec(
        vec![typ::list(typ_bool.clone())],
        vec![iterated_var_exp("input", &typ_bool, ast::Iter::List, 20)],
        exp_output.clone(),
        vec![prem_binding],
    );

    let converted = algo::convert(&must_spec).expect("iterated binding conversion");
    let clause = function_clause(&converted);
    let [source_premise] = clause.node.premises.as_slice() else {
        panic!("bound-plus-bind guard must suppress the matching output guard");
    };
    assert_eq!(source_premise.span, span(24));
    let ast::PremKind::Iter(iterated) = &source_premise.node else {
        panic!("expected analyzed iterated binding premise");
    };
    assert_eq!(
        iterated
            .iter_prem
            .vars_bound
            .iter()
            .map(|var| var.id.node.as_str())
            .collect::<Vec<_>>(),
        vec!["input"]
    );
    assert_eq!(
        iterated
            .iter_prem
            .vars_bind
            .iter()
            .map(|var| var.id.node.as_str())
            .collect::<Vec<_>>(),
        vec!["output"]
    );
    assert_eq!(clause.node.expression, exp_output);
}

#[test]
fn test_conversion_traverses_relation_matches_paths_and_else_without_sibling_leaks() {
    let rule =
        |name: &str, input: ast::Exp, output: ast::Exp, premises: Vec<ast::Prem>, line: i64| {
            crate::phrase! { node:
            ast::RuleKind {
                id: id(name, line),
                not_exp: Mixfix::Seq(vec![Mixfix::Arg(input), Mixfix::Arg(output)]),
                prems: premises,
            }, span:
            span(line) }
        };
    let debug_index = |value: bool, line: i64| {
        crate::phrase! { node:
        ast::PremKind::Debug(ast::DebugPrem {
            exp: literal_index_exp(value, line),
        }), span:
        span(line) }
    };
    let match_rule = rule(
        "match_path",
        literal_index_exp(true, 10),
        literal_index_exp(true, 11),
        vec![],
        10,
    );
    let first_sibling = rule(
        "first_sibling",
        exp(ast::ExpKind::Bool(true), ast::TypKind::Bool, 20),
        literal_index_exp(false, 22),
        vec![debug_index(false, 21)],
        20,
    );
    let second_sibling = rule(
        "second_sibling",
        exp(ast::ExpKind::Bool(true), ast::TypKind::Bool, 30),
        literal_index_exp(false, 31),
        vec![],
        30,
    );
    let else_rule = rule(
        "else_path",
        literal_index_exp(true, 40),
        literal_index_exp(true, 42),
        vec![debug_index(false, 41)],
        40,
    );
    let spec = vec![crate::phrase! { node:
    ast::DefKind::Rel(ast::Rel {
        id: id("relation", 1),
        not_typ: crate::phrase! { node:
            Mixfix::Seq(vec![Mixfix::Arg(typ::bool()), Mixfix::Arg(typ::bool())]), span:
            span(1) },
        input_hint: InputHint::new(vec![0]),
        rule_groups: vec![
            crate::phrase! { node: (id("match_group", 9), vec![match_rule]), span:  span(9) },
            crate::phrase! { node:
                (id("sibling_group", 19), vec![first_sibling, second_sibling]), span:
                span(19) },
        ],
        else_group: Some(crate::phrase! { node: (id("else_group", 39), else_rule), span:  span(39) }),
        hints: vec![],
    }), span:
    span(1) }];

    let converted = algo::convert(&spec).expect("guarded relation conversion");
    let crate::lang::al::ast::DefKind::Rel(relation) = &converted[0].node else {
        panic!("expected relation definition");
    };
    let [match_group, sibling_group] = relation.rule_groups.as_slice() else {
        panic!("expected match and sibling groups in source order");
    };
    assert_eq!(match_group.span, span(9));
    assert_eq!(match_group.node.rule_match.exps_input[0].span, span(10));
    let [match_guard, match_source] = match_group.node.rule_match.prems.as_slice() else {
        panic!("expected a guard before the analyzed match premise");
    };
    assert_index_guard_span(match_guard, span(10));
    assert_eq!(match_source.span, span(10));
    assert!(match_group.node.rule_paths[0].prems.is_empty());

    let [first_path, second_path] = sibling_group.node.rule_paths.as_slice() else {
        panic!("expected both sibling paths in source order");
    };
    let [first_guard, first_source] = first_path.prems.as_slice() else {
        panic!("expected one path-local guard before the source premise");
    };
    assert_index_guard_span(first_guard, span(21));
    assert_eq!(first_source.span, span(21));
    assert!(matches!(first_source.node, ast::PremKind::Debug(_)));
    let [second_guard] = second_path.prems.as_slice() else {
        panic!("the first sibling must not guard the second sibling output");
    };
    assert_index_guard_span(second_guard, span(31));

    let else_group = relation.else_group.as_ref().expect("else group preserved");
    assert_eq!(else_group.span, span(39));
    assert_eq!(else_group.node.rule_match.exps_input[0].span, span(40));
    let [else_guard, else_source] = else_group.node.rule_path.prems.as_slice() else {
        panic!("expected else-path guard and source premise only");
    };
    assert_index_guard_span(else_guard, span(41));
    assert_eq!(else_source.span, span(41));
    assert!(matches!(else_source.node, ast::PremKind::Debug(_)));
}

#[test]
fn test_conversion_traverses_else_clauses_in_guard_order() {
    let typ_bool = typ::bool();
    let exp_argument = joint_iteration(&[("left", 50), ("right", 51)], ast::Iter::List, 50);
    let exp_output = joint_iteration(&[("left", 50), ("right", 51)], ast::Iter::List, 52);
    let prem_debug = crate::phrase! { node:
    ast::PremKind::Debug(ast::DebugPrem {
        exp: literal_index_exp(false, 53),
    }), span:
    span(53) };
    let else_clause = crate::phrase! { node:
    ast::ClauseKind {
        args: vec![exp_arg(exp_argument)],
        expression: exp_output.clone(),
        premises: vec![prem_debug],
    }, span:
    span(50) };
    let spec = vec![crate::phrase! { node:
    ast::DefKind::FuncDec(ast::FuncDec {
        id: id("otherwise", 49),
        tparams: vec![],
        params: vec![crate::phrase! { node:
            ast::ParamKind::Exp(typ::list(crate::phrase! { node:
                ast::TypKind::Tuple(vec![typ_bool.clone(), typ_bool]), span:
                span(49) })), span:
            span(49) }],
        typ: crate::phrase! { node: exp_output.note.clone(), span:  span(49) },
        clauses: vec![],
        else_clause: Some(else_clause),
        hints: vec![],
    }), span:
    span(49) }];

    let converted = algo::convert(&spec).expect("guarded else-clause conversion");
    let crate::lang::al::ast::DefKind::FuncDec(function) = &converted[0].node else {
        panic!("expected function definition");
    };
    let clause = function
        .else_clause
        .as_ref()
        .expect("else clause preserved");
    let [guard, source] = clause.node.premises.as_slice() else {
        panic!("expected guard before the else-clause source premise");
    };
    assert_index_guard_span(guard, span(53));
    assert_eq!(source.span, span(53));
    assert!(matches!(source.node, ast::PremKind::Debug(_)));
    assert_eq!(clause.node.expression, exp_output);
    assert_eq!(clause.span, span(50));
}
