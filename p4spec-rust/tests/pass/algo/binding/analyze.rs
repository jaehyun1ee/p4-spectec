use super::super::*;

#[test]
fn test_conversion_preserves_rule_paths_and_populates_antiunified_inputs_in_order() {
    let tuple = |left: bool, right: bool, line: i64| {
        exp(
            ast::ExpKind::Tuple(vec![
                exp(ast::ExpKind::Bool(left), ast::TypKind::Bool, line),
                exp(ast::ExpKind::Bool(right), ast::TypKind::Bool, line + 1),
            ]),
            ast::TypKind::Tuple(vec![typ::bool(), typ::bool()]),
            line,
        )
    };
    let relation_not_typ = crate::phrase! { node:
    Mixfix::Seq(vec![
        Mixfix::Arg(crate::phrase! { node:
            ast::TypKind::Tuple(vec![typ::bool(), typ::bool()]), span:
            span(1) }),
        Mixfix::Arg(typ::bool()),
    ]), span:
    span(1) };
    let rule = |name: &str, input: ast::Exp, output: bool, line: i64| {
        crate::phrase! { node:
        ast::RuleKind {
            id: id(name, line),
            not_exp: Mixfix::Seq(vec![
                Mixfix::Arg(input),
                Mixfix::Arg(exp(ast::ExpKind::Bool(output), ast::TypKind::Bool, line)),
            ]),
            prems: vec![],
        }, span:
        span(line) }
    };
    let rules_first = vec![
        rule("first", tuple(true, false, 2), true, 2),
        rule("second", tuple(false, true, 5), false, 5),
    ];
    let rules_second = vec![rule("third", tuple(true, true, 8), false, 8)];
    let spec = vec![crate::phrase! { node:
    ast::DefKind::Rel(ast::Rel {
        id: id("relation", 1),
        not_typ: relation_not_typ,
        input_hint: InputHint::new(vec![0]),
        rule_groups: vec![
            crate::phrase! { node: (id("first_group", 1), rules_first), span:  span(1) },
            crate::phrase! { node: (id("second_group", 8), rules_second), span:  span(8) },
        ],
        else_group: None,
        hints: vec![],
    }), span:
    span(1) }];

    let analyzed = algo::convert(spec).expect("convertible relation");

    let crate::lang::al::ast::DefKind::Rel(relation) = &analyzed[0].node else {
        panic!("expected relation definition");
    };
    let [rule_group, second_group] = relation.rule_groups.as_slice() else {
        panic!("expected two rule groups");
    };
    assert_eq!(rule_group.node.id.node, "first_group");
    assert_eq!(second_group.node.id.node, "second_group");
    assert_eq!(second_group.node.rule_paths[0].id.node, "third");
    assert_eq!(
        rule_group
            .node
            .rule_paths
            .iter()
            .map(|path| path.id.node.as_str())
            .collect::<Vec<_>>(),
        vec!["first", "second"]
    );
    let ast::ExpKind::Tuple(items) = &rule_group.node.rule_match.exps_signature[0].node else {
        panic!("expected tuple rule signature");
    };
    assert!(
        items
            .iter()
            .all(|item| matches!(item.node, ast::ExpKind::Var(_)))
    );
    let compared_values = |prems: &[ast_al::Prem]| {
        prems
            .iter()
            .map(|prem| {
                let ast_al::PremKind::If(if_prem) = &prem.node else {
                    panic!("expected populated equality premise");
                };
                let ast::ExpKind::Cmp(_, _, _, exp_r) = &if_prem.exp.node else {
                    panic!("expected equality comparison");
                };
                let ast::ExpKind::Bool(value) = exp_r.node else {
                    panic!("expected original boolean input");
                };
                value
            })
            .collect::<Vec<_>>()
    };
    assert_eq!(
        compared_values(&rule_group.node.rule_paths[0].prems),
        vec![true, false]
    );
    assert_eq!(
        compared_values(&rule_group.node.rule_paths[1].prems),
        vec![false, true]
    );
    assert!(matches!(
        rule_group.node.rule_paths[0].exps_output[0].node,
        ast::ExpKind::Bool(true)
    ));
    assert!(matches!(
        rule_group.node.rule_paths[1].exps_output[0].node,
        ast::ExpKind::Bool(false)
    ));
}

#[test]
fn test_clause_analysis_orders_partial_then_repeated_then_source_premises() {
    let tuple_typ = crate::phrase! { node:
    ast::TypKind::Tuple(vec![typ::bool(), typ::bool(), typ::bool()]), span:
    span(1) };
    let tuple = exp(
        ast::ExpKind::Tuple(vec![
            var_exp("x", 2),
            var_exp("x", 3),
            exp(ast::ExpKind::Bool(true), ast::TypKind::Bool, 4),
        ]),
        tuple_typ.node.clone(),
        2,
    );
    let clause = crate::phrase! { node:
    ast::ClauseKind {
        args: vec![crate::phrase! { node: ast::ArgKind::Exp(Box::new(tuple)), span:  span(2) }],
        expression: var_exp("x", 5),
        premises: vec![crate::phrase! { node:
            ast::PremKind::Debug(ast::DebugPrem {
                exp: exp(ast::ExpKind::Bool(false), ast::TypKind::Bool, 6),
            }), span:
            span(6) }],
    }, span:
    span(2) };
    let spec = vec![crate::phrase! { node:
    ast::DefKind::FuncDec(ast::FuncDec {
        id: id("function", 1),
        tparams: vec![],
        params: vec![crate::phrase! { node: ast::ParamKind::Exp(tuple_typ), span:  span(1) }],
        typ: typ::bool(),
        clauses: vec![clause],
        else_clause: None,
        hints: vec![],
    }), span:
    span(1) }];

    let analyzed = algo::convert(spec).expect("convertible function");

    let crate::lang::al::ast::DefKind::FuncDec(function) = &analyzed[0].node else {
        panic!("expected function definition");
    };
    let prems = &function.clauses[0].node.premises;
    assert_eq!(prems.len(), 3);
    assert!(matches!(
        &prems[0].node,
        ast_al::PremKind::If(ast_al::IfPrem {
            exp: NotePhrase {
                node: ast::ExpKind::Cmp(_, _, _, exp_r),
                ..
            }
        }) if matches!(exp_r.node, ast::ExpKind::Bool(true))
    ));
    assert!(matches!(
        &prems[1].node,
        ast_al::PremKind::If(ast_al::IfPrem {
            exp: NotePhrase {
                node: ast::ExpKind::Cmp(_, _, _, exp_r),
                ..
            }
        }) if matches!(exp_r.node, ast::ExpKind::Var(_))
    ));
    assert!(matches!(&prems[2].node, ast_al::PremKind::Debug(_)));
}

#[test]
fn test_otherwise_clauses_and_rules_reject_impure_premises_at_the_branch_span() {
    let impure_premise = |line: i64| {
        crate::phrase! { node:
        ast::PremKind::If(ast::IfPrem {
            exp: exp(ast::ExpKind::Bool(true), ast::TypKind::Bool, line),
        }), span:
        span(line) }
    };
    let else_clause = crate::phrase! { node:
    ast::ClauseKind {
        args: vec![crate::phrase! { node:
            ast::ArgKind::Exp(Box::new(var_exp("x", 10))), span:
            span(10) }],
        expression: var_exp("x", 10),
        premises: vec![impure_premise(11)],
    }, span:
    span(10) };
    let function_spec = vec![crate::phrase! { node:
    ast::DefKind::FuncDec(ast::FuncDec {
        id: id("function", 9),
        tparams: vec![],
        params: vec![crate::phrase! { node: ast::ParamKind::Exp(typ::bool()), span:  span(9) }],
        typ: typ::bool(),
        clauses: vec![],
        else_clause: Some(else_clause),
        hints: vec![],
    }), span:
    span(9) }];

    let function_error = algo::convert(function_spec).expect_err("impure otherwise clause");
    assert_eq!(function_error.kind, AlgoErrorKind::ImpureElsePremises);
    assert_eq!(function_error.span, span(10));

    let relation_not_typ = crate::phrase! { node: Mixfix::Arg(typ::bool()), span:  span(20) };
    let else_rule = crate::phrase! { node:
    ast::RuleKind {
        id: id("else_rule", 21),
        not_exp: Mixfix::Arg(var_exp("input", 21)),
        prems: vec![impure_premise(22)],
    }, span:
    span(21) };
    let relation_spec = vec![crate::phrase! { node:
    ast::DefKind::Rel(ast::Rel {
        id: id("relation", 20),
        not_typ: relation_not_typ,
        input_hint: InputHint::new(vec![0]),
        rule_groups: vec![],
        else_group: Some(crate::phrase! { node: (id("else_group", 20), else_rule), span:  span(20) }),
        hints: vec![],
    }), span:
    span(20) }];

    let relation_error = algo::convert(relation_spec).expect_err("impure otherwise rule");
    assert_eq!(relation_error.kind, AlgoErrorKind::ImpureElsePremises);
    assert_eq!(relation_error.span, span(21));
}

#[test]
fn test_conversion_rejects_overlapping_and_missing_variant_table_patterns() {
    let choice_id = id("Choice", 1);
    let choice_typ =
        crate::phrase! { node: ast::TypKind::Var(choice_id.clone(), vec![]), span:  span(1) };
    let origin = crate::phrase! { node: (choice_id.clone(), vec![]), span:  span(1) };
    let choice_def = crate::phrase! { node:
    ast::DefKind::Typ(ast::TypDef {
        id: choice_id,
        tparams: vec![],
        def_typ: crate::phrase! { node:
            ast::DefTypKind::Variant(vec![
                (not_typ("A", 1), origin.clone(), vec![]),
                (not_typ("B", 1), origin, vec![]),
            ]), span:
            span(1) },
        hints: vec![],
    }), span:
    span(1) };
    let table = |rows: Vec<ast::TableRow>, line: i64| {
        crate::phrase! { node:
        ast::DefKind::TableDec(ast::TableDec {
            id: id("table", line),
            params: vec![crate::phrase! { node:
                ast::ParamKind::Exp(choice_typ.clone()), span:
                span(line) }],
            typ: typ::bool(),
            rows,
            hints: vec![],
        }), span:
        span(line - 1) }
    };
    let row = |pattern: ast::Exp, line: i64| {
        crate::phrase! { node:
        (
            vec![crate::phrase! { node:
                ast::ArgKind::Exp(Box::new(pattern)), span:
                span(line) }],
            exp(ast::ExpKind::Bool(true), ast::TypKind::Bool, line),
        ), span:
        span(line) }
    };
    let case_pattern = |name: &str, line: i64| {
        let keyword = crate::phrase! { node: Atom::Keyword(name.to_owned()), span:  span(line) };
        let case = exp(
            ast::ExpKind::Case(Box::new(Mixfix::Atom(keyword))),
            choice_typ.node.clone(),
            line,
        );
        exp(
            ast::ExpKind::UpCast(choice_typ.clone(), Box::new(case)),
            choice_typ.node.clone(),
            line,
        )
    };
    let overlap_spec = vec![
        choice_def.clone(),
        table(
            vec![
                row(case_pattern("A", 10), 10),
                row(case_pattern("A", 11), 11),
            ],
            9,
        ),
    ];

    let overlap_error =
        algo::convert(overlap_spec).expect_err("overlap takes precedence over missing");
    assert_eq!(overlap_error.kind, AlgoErrorKind::OverlappingTablePatterns);
    assert_eq!(overlap_error.span, span(8));

    let missing_spec = vec![choice_def, table(vec![row(case_pattern("A", 20), 20)], 19)];

    let missing_error = algo::convert(missing_spec).expect_err("missing B variant row");
    assert_eq!(missing_error.kind, AlgoErrorKind::MissingTablePatterns);
    assert_eq!(missing_error.span, span(18));
}

#[test]
fn test_conversion_preserves_definition_clause_and_table_row_order() {
    let choice_id = id("Choice", 2);
    let choice_typ =
        crate::phrase! { node: ast::TypKind::Var(choice_id.clone(), vec![]), span:  span(2) };
    let origin = crate::phrase! { node: (choice_id.clone(), vec![]), span:  span(2) };
    let choice_def = crate::phrase! { node:
    ast::DefKind::Typ(ast::TypDef {
        id: choice_id,
        tparams: vec![],
        def_typ: crate::phrase! { node:
            ast::DefTypKind::Variant(vec![
                (not_typ("A", 2), origin.clone(), vec![]),
                (not_typ("B", 2), origin, vec![]),
            ]), span:
            span(2) },
        hints: vec![],
    }), span:
    span(2) };
    let clause = |name: &str, line: i64| {
        crate::phrase! { node:
        ast::ClauseKind {
            args: vec![crate::phrase! { node:
                ast::ArgKind::Exp(Box::new(var_exp(name, line))), span:
                span(line) }],
            expression: var_exp(name, line),
            premises: vec![],
        }, span:
        span(line) }
    };
    let function_def = crate::phrase! { node:
    ast::DefKind::FuncDec(ast::FuncDec {
        id: id("function", 3),
        tparams: vec![],
        params: vec![crate::phrase! { node: ast::ParamKind::Exp(typ::bool()), span:  span(3) }],
        typ: typ::bool(),
        clauses: vec![clause("first_clause", 4), clause("second_clause", 5)],
        else_clause: None,
        hints: vec![],
    }), span:
    span(3) };
    let row = |name: &str, value: bool, line: i64| {
        let pattern = exp(
            ast::ExpKind::Var(id(name, line)),
            choice_typ.node.clone(),
            line,
        );
        crate::phrase! { node:
        (
            vec![crate::phrase! { node:
                ast::ArgKind::Exp(Box::new(pattern)), span:
                span(line) }],
            literal_index_exp(value, line),
        ), span:
        span(line) }
    };
    let table_def = crate::phrase! { node:
    ast::DefKind::TableDec(ast::TableDec {
        id: id("table", 6),
        params: vec![crate::phrase! { node:
            ast::ParamKind::Exp(choice_typ.clone()), span:
            span(6) }],
        typ: typ::bool(),
        rows: vec![row("specific", true, 7), row("_closer", false, 8)],
        hints: vec![],
    }), span:
    span(6) };
    let variable_def = crate::phrase! { node:
    ast::DefKind::Var(ast::VarDef {
        id: id("variable", 9),
        typ: typ::bool(),
        hints: vec![],
    }), span:
    span(9) };
    let extern_relation_def = crate::phrase! { node:
    ast::DefKind::ExternRel(ast::ExternRel {
        id: id("external_relation", 10),
        not_typ: crate::phrase! { node: Mixfix::Arg(typ::bool()), span:  span(10) },
        input_hint: InputHint::new(vec![0]),
        hints: vec![],
    }), span:
    span(10) };
    let extern_dec_def = crate::phrase! { node:
    ast::DefKind::ExternDec(ast::ExternDec {
        id: id("external_dec", 11),
        tparams: vec![],
        params: vec![crate::phrase! { node: ast::ParamKind::Exp(typ::bool()), span:  span(11) }],
        typ: typ::bool(),
        hints: vec![],
    }), span:
    span(11) };
    let builtin_dec_def = crate::phrase! { node:
    ast::DefKind::BuiltinDec(ast::BuiltinDec {
        id: id("builtin_dec", 12),
        tparams: vec![],
        params: vec![crate::phrase! { node: ast::ParamKind::Exp(typ::bool()), span:  span(12) }],
        typ: typ::bool(),
        hints: vec![],
    }), span:
    span(12) };
    let spec = vec![
        crate::phrase! { node:
        ast::DefKind::ExternTyp(ast::ExternTyp {
            id: id("external", 1),
            hints: vec![],
        }), span:
        span(1) },
        variable_def,
        extern_relation_def,
        choice_def,
        extern_dec_def,
        builtin_dec_def,
        function_def,
        table_def,
    ];

    let analyzed = algo::convert(spec).expect("ordered specification");

    let definition_ids = analyzed
        .iter()
        .map(|def| match &def.node {
            crate::lang::al::ast::DefKind::ExternTyp(def) => def.id.node.as_str(),
            crate::lang::al::ast::DefKind::Var(def) => def.id.node.as_str(),
            crate::lang::al::ast::DefKind::ExternRel(def) => def.id.node.as_str(),
            crate::lang::al::ast::DefKind::Rel(def) => def.id.node.as_str(),
            crate::lang::al::ast::DefKind::Typ(def) => def.id.node.as_str(),
            crate::lang::al::ast::DefKind::ExternDec(def) => def.id.node.as_str(),
            crate::lang::al::ast::DefKind::BuiltinDec(def) => def.id.node.as_str(),
            crate::lang::al::ast::DefKind::FuncDec(def) => def.id.node.as_str(),
            crate::lang::al::ast::DefKind::TableDec(def) => def.id.node.as_str(),
        })
        .collect::<Vec<_>>();
    assert_eq!(
        definition_ids,
        vec![
            "external",
            "variable",
            "external_relation",
            "Choice",
            "external_dec",
            "builtin_dec",
            "function",
            "table",
        ]
    );

    let crate::lang::al::ast::DefKind::FuncDec(function) = &analyzed[6].node else {
        panic!("expected function definition");
    };
    let clause_ids = function
        .clauses
        .iter()
        .map(|clause| {
            let ast::ArgKind::Exp(exp) = &clause.node.args[0].node else {
                panic!("expected expression argument");
            };
            let ast::ExpKind::Var(id) = &exp.node else {
                panic!("expected variable argument");
            };
            id.node.as_str()
        })
        .collect::<Vec<_>>();
    assert_eq!(clause_ids, vec!["first_clause", "second_clause"]);

    let crate::lang::al::ast::DefKind::TableDec(table) = &analyzed[7].node else {
        panic!("expected table definition");
    };
    let row_ids = table
        .table_rows
        .iter()
        .map(|row| {
            let ast::ExpKind::Var(id) = &row.node.exps_signature[0].node else {
                panic!("expected variable signature");
            };
            id.node.as_str()
        })
        .collect::<Vec<_>>();
    assert_eq!(row_ids, vec!["specific", "_closer"]);
    assert_eq!(
        table
            .table_rows
            .iter()
            .map(|row| row.span.clone())
            .collect::<Vec<_>>(),
        vec![span(7), span(8)]
    );
    assert!(table.table_rows.iter().all(|row| {
        row.node.prems.is_empty() && matches!(row.node.exp.node, ast::ExpKind::Idx(_, _))
    }));
}
