use p4spec_rust::{
    lang::{
        common::{
            Id,
            notation::{atom::Atom, mixfix::Mixfix},
            noted::Noted,
            source::{Position, Span, Spanned},
        },
        hints::input::InputHint,
        il::ast,
        traits::eq::SyntaxEq,
        xl,
    },
    pass::algo::{
        self, AlgoErrorKind,
        binding::{
            analyze, antiunify,
            bind::{self, Binding, Bindings},
            collect,
            context::Context,
            dimension,
            iteration::IterationContext,
            multiple, partial,
            pattern::{self, PatternSet, PatternSets},
            shallow,
        },
    },
    runtime::{
        sta::Dim,
        types::{TypeDef, typ},
    },
};

fn span(line: i64) -> Span {
    let position = Position::new("algorithmic.watsup", line, 0);
    Span::new(position.clone(), position)
}

fn id(name: &str, line: i64) -> Id {
    Spanned::new(name.to_owned(), span(line))
}

fn exp(kind: ast::ExpKind, note: ast::TypKind, line: i64) -> ast::Exp {
    Spanned::new(Noted::new(kind, note), span(line))
}

fn var_exp(name: &str, line: i64) -> ast::Exp {
    exp(ast::ExpKind::Var(id(name, line)), ast::TypKind::Bool, line)
}

fn not_typ(name: &str, line: i64) -> ast::NotTyp {
    let atom = Spanned::new(Atom::Keyword(name.to_owned()), span(line));
    Spanned::new(Mixfix::Atom(atom), span(line))
}

fn pattern_set(names: &[&str]) -> PatternSet {
    names
        .iter()
        .enumerate()
        .map(|(index, name)| not_typ(name, index as i64 + 1))
        .collect()
}

#[test]
fn unsupported_conversion_uses_the_first_definition_span() {
    let spec = vec![
        Spanned::new(
            ast::DefKind::ExternTyp(ast::ExternTyp {
                id: id("first", 41),
                hints: vec![],
            }),
            span(41),
        ),
        Spanned::new(
            ast::DefKind::ExternTyp(ast::ExternTyp {
                id: id("last", 43),
                hints: vec![],
            }),
            span(43),
        ),
    ];
    let error = algo::convert(&spec).expect_err("foundation conversion stub");

    assert_eq!(error.kind, AlgoErrorKind::Unsupported);
    assert_eq!(error.span, span(41));
}

#[test]
fn context_loads_type_and_metavariable_definitions() {
    let extern_id = id("extern_type", 1);
    let defined_id = id("defined_type", 2);
    let variable_id = id("value", 3);
    let bool_typ = Spanned::new(ast::TypKind::Bool, span(2));
    let def_typ = Spanned::new(ast::DefTypKind::Plain(bool_typ.clone()), span(2));
    let spec = vec![
        Spanned::new(
            ast::DefKind::ExternTyp(ast::ExternTyp {
                id: extern_id.clone(),
                hints: vec![],
            }),
            span(1),
        ),
        Spanned::new(
            ast::DefKind::Typ(ast::TypDef {
                id: defined_id.clone(),
                tparams: vec![],
                def_typ: def_typ.clone(),
                hints: vec![],
            }),
            span(2),
        ),
        Spanned::new(
            ast::DefKind::Var(ast::VarDef {
                id: variable_id.clone(),
                typ: bool_typ.clone(),
                hints: vec![],
            }),
            span(3),
        ),
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

#[test]
fn binding_union_keeps_the_first_span_and_marks_repetition() {
    let id_first = id("x", 1);
    let id_second = id("x", 2);
    let dim = Dim::new(typ::bool(), vec![]);
    let mut bindings_l = Bindings::new();
    bindings_l.insert(id_first.clone(), Binding::Single(dim.clone()));
    let mut bindings_r = Bindings::new();
    bindings_r.insert(id_second, Binding::Single(dim.clone()));

    let bindings = bind::union(bindings_l, bindings_r).expect("equivalent dimensions");

    assert_eq!(bindings.keys().next(), Some(&id_first));
    let Binding::Multiple(actual) = bindings.get(&id_first).expect("merged binding") else {
        panic!("expected a repeated binding");
    };
    assert!(actual.equiv(&dim));
}

#[test]
fn binding_union_rejects_conflicting_dimensions_at_the_first_key() {
    let id_first = id("x", 4);
    let id_second = id("x", 8);
    let mut bindings_l = Bindings::new();
    bindings_l.insert(
        id_first.clone(),
        Binding::Single(Dim::new(typ::bool(), vec![])),
    );
    let mut bindings_r = Bindings::new();
    bindings_r.insert(
        id_second,
        Binding::Single(Dim::new(typ::bool(), vec![ast::Iter::List])),
    );

    let error = bind::union(bindings_l, bindings_r).expect_err("conflicting dimensions");

    assert_eq!(error.kind, AlgoErrorKind::InconsistentDimensions);
    assert_eq!(error.span, id_first.span);
}

#[test]
fn dimension_inference_keeps_the_minimal_occurrence() {
    let iterated_var = var_exp("x", 2);
    let iterated = exp(
        ast::ExpKind::Iter(Box::new(iterated_var), (ast::Iter::List, vec![])),
        ast::TypKind::Iter(Box::new(typ::bool()), ast::Iter::List),
        3,
    );
    let direct = var_exp("x", 4);
    let tuple = exp(
        ast::ExpKind::Tuple(vec![iterated, direct]),
        ast::TypKind::Tuple(vec![]),
        1,
    );

    let dimensions = dimension::infer_exp(&tuple);
    let (stored_id, actual) = dimensions.iter().next().expect("inferred variable");

    assert_eq!(stored_id.span, span(2));
    assert!(actual.equiv(&Dim::new(typ::bool(), vec![])));
}

#[test]
fn collection_rejects_a_binding_inside_a_noninvertible_operator() {
    let variable = var_exp("x", 7);
    let negated = exp(
        ast::ExpKind::Un(
            ast::UnOp::Bool(xl::bool::UnOp::Not),
            ast::OpTyp::Bool,
            Box::new(variable),
        ),
        ast::TypKind::Bool,
        6,
    );

    let error =
        collect::collect_exp(&Context::new(), &negated).expect_err("binding under unary operator");

    assert_eq!(
        error.kind,
        AlgoErrorKind::NonInvertibleBinding("unary operator")
    );
    assert_eq!(error.span, span(7));
}

#[test]
fn expression_collection_reports_right_associated_conflict_span() {
    let iterated = exp(
        ast::ExpKind::Iter(Box::new(var_exp("x", 3)), (ast::Iter::List, vec![])),
        ast::TypKind::Iter(Box::new(typ::bool()), ast::Iter::List),
        3,
    );
    let tuple = exp(
        ast::ExpKind::Tuple(vec![var_exp("x", 1), var_exp("x", 2), iterated]),
        ast::TypKind::Tuple(vec![]),
        1,
    );

    let error = collect::collect_exp(&Context::new(), &tuple)
        .expect_err("third occurrence conflicts with the repeated tail binding");

    assert_eq!(error.kind, AlgoErrorKind::InconsistentDimensions);
    assert_eq!(error.span, span(2));
}

#[test]
fn argument_collection_reports_right_associated_conflict_span() {
    let iterated = exp(
        ast::ExpKind::Iter(Box::new(var_exp("x", 3)), (ast::Iter::List, vec![])),
        ast::TypKind::Iter(Box::new(typ::bool()), ast::Iter::List),
        3,
    );
    let args = [var_exp("x", 1), var_exp("x", 2), iterated]
        .into_iter()
        .map(|exp| Spanned::new(ast::ArgKind::Exp(Box::new(exp)), span(1)))
        .collect::<Vec<_>>();

    let error = collect::collect_args(&Context::new(), &args)
        .expect_err("third occurrence conflicts with the repeated tail binding");

    assert_eq!(error.kind, AlgoErrorKind::InconsistentDimensions);
    assert_eq!(error.span, span(2));
}

#[test]
fn shallow_cases_accept_only_iterated_variables_as_arguments() {
    let variable = var_exp("x", 1);
    let iterated = exp(
        ast::ExpKind::Iter(Box::new(variable), (ast::Iter::List, vec![])),
        ast::TypKind::Iter(Box::new(typ::bool()), ast::Iter::List),
        1,
    );
    let shallow_case = exp(
        ast::ExpKind::Case(Box::new(Mixfix::Arg(iterated))),
        ast::TypKind::Bool,
        1,
    );
    let nested_tuple = exp(
        ast::ExpKind::Tuple(vec![var_exp("x", 2)]),
        ast::TypKind::Tuple(vec![typ::bool()]),
        2,
    );
    let deep_case = exp(
        ast::ExpKind::Case(Box::new(Mixfix::Arg(nested_tuple))),
        ast::TypKind::Bool,
        2,
    );

    assert!(shallow::check_exp(&shallow_case));
    assert!(!shallow::check_exp(&deep_case));
}

#[test]
fn pattern_overlap_requires_intersection_in_every_dimension() {
    let owner_span = span(1);
    let pattern_a: PatternSets = vec![pattern_set(&["A", "B"]), pattern_set(&["X"])];
    let pattern_b: PatternSets = vec![pattern_set(&["B"]), pattern_set(&["X", "Y"])];
    let pattern_c: PatternSets = vec![pattern_set(&["B"]), pattern_set(&["Y"])];

    assert!(pattern::has_overlap(&owner_span, &pattern_a, &pattern_b).expect("matching arity"));
    assert!(!pattern::has_overlap(&owner_span, &pattern_a, &pattern_c).expect("matching arity"));
}

#[test]
fn pattern_arity_errors_use_the_owning_source_span() {
    let owner_span = span(31);
    let patterns_l: PatternSets = vec![pattern_set(&["A"])];
    let patterns_r: PatternSets = vec![pattern_set(&["A"]), pattern_set(&["B"])];

    let error = pattern::has_overlap(&owner_span, &patterns_l, &patterns_r)
        .expect_err("different pattern arities");

    assert_eq!(
        error.kind,
        AlgoErrorKind::PatternArityMismatch {
            expected: 1,
            actual: 2,
        }
    );
    assert_eq!(error.span, owner_span);
}

#[test]
fn pattern_sets_order_mixfix_structure_before_rendered_text() {
    let argument = Spanned::new(Mixfix::Arg(typ::bool()), span(2));
    let atom = not_typ("A", 1);
    let patterns: PatternSet = [atom, argument].into_iter().collect();
    let ordered = patterns.iter().collect::<Vec<_>>();

    assert!(matches!(ordered[0].node, Mixfix::Arg(_)));
    assert!(matches!(ordered[1].node, Mixfix::Atom(_)));
}

#[test]
fn pattern_subtraction_preserves_cartesian_fragment_order() {
    let owner_span = span(1);
    let total: PatternSets = vec![pattern_set(&["A", "B"]), pattern_set(&["X", "Y"])];
    let covered: PatternSets = vec![pattern_set(&["A"]), pattern_set(&["X"])];

    let missing = pattern::subtract(&owner_span, &total, &covered).expect("matching arity");

    assert_eq!(
        missing,
        vec![
            vec![pattern_set(&["B"]), pattern_set(&["X", "Y"])],
            vec![pattern_set(&["A"]), pattern_set(&["Y"])],
        ]
    );
}

#[test]
fn multiple_binding_renames_repetitions_and_compares_them_in_occurrence_order() {
    let tuple = exp(
        ast::ExpKind::Tuple(vec![var_exp("x", 1), var_exp("x", 2), var_exp("x", 3)]),
        ast::TypKind::Tuple(vec![typ::bool(), typ::bool(), typ::bool()]),
        1,
    );
    let bindings = collect::collect_exp(&Context::new(), &tuple).expect("binding collection");
    let mut context = Context::new();
    let mut renames = multiple::RenameEnv::from_bindings(&bindings);

    let renamed = multiple::rename_exp(&mut context, &mut renames, &tuple);
    let side_conditions =
        multiple::generate_side_conditions(&bindings, &IterationContext::new(), &renames);

    let ast::ExpKind::Tuple(exps) = &renamed.node.kind else {
        panic!("expected tuple binding");
    };
    let ids = exps
        .iter()
        .map(|exp| match &exp.node.kind {
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
    let ast::PremKind::If(if_prem) = &side_condition.node else {
        panic!("expected conditional premise");
    };
    let ast::ExpKind::Bin(_, ast::OpTyp::Bool, first, second) = &if_prem.exp.node.kind else {
        panic!("expected ordered equality conjunction");
    };
    let compared_span = |exp: &ast::Exp| {
        let ast::ExpKind::Cmp(_, ast::OpTyp::Bool, _, exp_r) = &exp.node.kind else {
            panic!("expected equality comparison");
        };
        let ast::ExpKind::Var(id) = &exp_r.node.kind else {
            panic!("expected renamed right operand");
        };
        id.span.clone()
    };
    assert_eq!(compared_span(first), span(2));
    assert_eq!(compared_span(second), span(3));
}

#[test]
fn partial_binding_preserves_expression_and_premise_iteration_dimensions() {
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
            Box::new(Spanned::new(
                ast::TypKind::Tuple(vec![typ::bool(), typ::bool()]),
                span(1),
            )),
            ast::Iter::List,
        ),
        1,
    );
    let mut context = Context::new();
    let bindings = collect::collect_exp(&context, &iterated).expect("binding collection");
    let mut renames = partial::RenameEnv::new();

    let (_, renamed) = partial::rename_exp(
        &mut context,
        &bindings.domain(),
        &mut renames,
        IterationContext::new(),
        &iterated,
    )
    .expect("partial binding rename");
    let premises = partial::generate_prems(&context, &IterationContext::new(), &renames)
        .expect("partial binding premises");

    let ast::ExpKind::Iter(exp_inner, (ast::Iter::List, vars)) = &renamed.node.kind else {
        panic!("expected iterated binding");
    };
    let ast::ExpKind::Tuple(exps) = &exp_inner.node.kind else {
        panic!("expected tuple binding");
    };
    let ast::ExpKind::Var(id_rename) = &exps[1].node.kind else {
        panic!("expected bound value to be renamed");
    };
    assert_eq!(vars.len(), 2);
    assert_eq!(vars[0].id.node, "x");
    assert_eq!(vars[1].id, *id_rename);

    let [premise] = premises.as_slice() else {
        panic!("expected one equality premise");
    };
    let ast::PremKind::Iter(iterated_prem) = &premise.node else {
        panic!("expected premise iteration");
    };
    assert_eq!(iterated_prem.iter_prem.iter, ast::Iter::List);
    assert_eq!(iterated_prem.iter_prem.vars_bound.len(), 1);
    assert_eq!(iterated_prem.iter_prem.vars_bound[0].id, *id_rename);
    let ast::PremKind::If(if_prem) = &iterated_prem.prem.node else {
        panic!("expected equality side condition");
    };
    let ast::ExpKind::Cmp(_, ast::OpTyp::Bool, exp_l, exp_r) = &if_prem.exp.node.kind else {
        panic!("expected equality comparison");
    };
    assert!(matches!(&exp_l.node.kind, ast::ExpKind::Var(id) if id == id_rename));
    assert!(matches!(exp_r.node.kind, ast::ExpKind::Bool(true)));
}

#[test]
fn partial_case_and_list_bindings_generate_match_then_bind_premises_in_source_order() {
    let choice_id = id("Choice", 1);
    let choice_typ = Spanned::new(ast::TypKind::Var(choice_id.clone(), vec![]), span(1));
    let origin = Spanned::new((choice_id.clone(), vec![]), span(1));
    let def_typ = Spanned::new(
        ast::DefTypKind::Variant(vec![
            (not_typ("A", 1), origin.clone(), vec![]),
            (not_typ("B", 1), origin, vec![]),
        ]),
        span(1),
    );
    let mut context = Context::new();
    context
        .tdenv
        .insert(choice_id, TypeDef::Defined(vec![], Box::new(def_typ)));

    let keyword = Spanned::new(Atom::Keyword("A".to_owned()), span(2));
    let case = exp(
        ast::ExpKind::Case(Box::new(Mixfix::Seq(vec![
            Mixfix::Atom(keyword),
            Mixfix::Arg(var_exp("y", 2)),
        ]))),
        choice_typ.node.clone(),
        2,
    );
    let list_typ = Spanned::new(
        ast::TypKind::Iter(Box::new(typ::bool()), ast::Iter::List),
        span(3),
    );
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
    let bindings = collect::collect_exp(&context, &tuple).expect("binding collection");
    let mut renames = partial::RenameEnv::new();

    let (_, renamed) = partial::rename_exp(
        &mut context,
        &bindings.domain(),
        &mut renames,
        IterationContext::new(),
        &tuple,
    )
    .expect("partial binding rename");
    let premises = partial::generate_prems(&context, &IterationContext::new(), &renames)
        .expect("partial binding premises");

    let ast::ExpKind::Tuple(exps) = &renamed.node.kind else {
        panic!("expected tuple binding");
    };
    assert!(matches!(exps[0].node.kind, ast::ExpKind::Var(_)));
    assert!(matches!(exps[1].node.kind, ast::ExpKind::Iter(_, _)));
    assert_eq!(premises.len(), 4);
    assert!(matches!(
        &premises[0].node,
        ast::PremKind::If(ast::IfPrem {
            exp: Spanned {
                node: Noted {
                    kind: ast::ExpKind::Match(_, ast::Pattern::Case(_)),
                    ..
                },
                ..
            }
        })
    ));
    assert!(matches!(
        &premises[1].node,
        ast::PremKind::Let(ast::LetPrem {
            exp_l: Spanned {
                node: Noted {
                    kind: ast::ExpKind::Case(_),
                    ..
                },
                ..
            },
            ..
        })
    ));
    assert!(matches!(
        &premises[2].node,
        ast::PremKind::If(ast::IfPrem {
            exp: Spanned {
                node: Noted {
                    kind: ast::ExpKind::Match(_, ast::Pattern::List(ast::ListPattern::Fixed(1))),
                    ..
                },
                ..
            }
        })
    ));
    assert!(matches!(
        &premises[3].node,
        ast::PremKind::Let(ast::LetPrem {
            exp_l: Spanned {
                node: Noted {
                    kind: ast::ExpKind::List(_),
                    ..
                },
                ..
            },
            ..
        })
    ));
}

#[test]
fn partial_upcast_binding_checks_subtype_before_binding_the_downcast_value() {
    let parent_id = id("Parent", 1);
    let child_id = id("Child", 1);
    let parent_typ = Spanned::new(ast::TypKind::Var(parent_id.clone(), vec![]), span(1));
    let child_typ = Spanned::new(ast::TypKind::Var(child_id.clone(), vec![]), span(1));
    let parent_origin = Spanned::new((parent_id.clone(), vec![]), span(1));
    let child_origin = Spanned::new((child_id.clone(), vec![]), span(1));
    let mut context = Context::new();
    context.tdenv.insert(
        parent_id,
        TypeDef::Defined(
            vec![],
            Box::new(Spanned::new(
                ast::DefTypKind::Variant(vec![
                    (not_typ("A", 1), parent_origin.clone(), vec![]),
                    (not_typ("B", 1), parent_origin, vec![]),
                ]),
                span(1),
            )),
        ),
    );
    context.tdenv.insert(
        child_id,
        TypeDef::Defined(
            vec![],
            Box::new(Spanned::new(
                ast::DefTypKind::Variant(vec![(not_typ("A", 1), child_origin, vec![])]),
                span(1),
            )),
        ),
    );
    let child_var = exp(ast::ExpKind::Var(id("child", 2)), child_typ.node.clone(), 2);
    let upcast = exp(
        ast::ExpKind::UpCast(parent_typ.clone(), Box::new(child_var)),
        parent_typ.node.clone(),
        2,
    );
    let bindings = collect::collect_exp(&context, &upcast).expect("binding collection");
    let mut renames = partial::RenameEnv::new();

    partial::rename_exp(
        &mut context,
        &bindings.domain(),
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
        ast::PremKind::If(ast::IfPrem {
            exp: Spanned {
                node: Noted {
                    kind: ast::ExpKind::Sub(_, typ, _),
                    ..
                },
                ..
            }
        }) if typ.syntax_eq(&child_typ)
    ));
    assert!(matches!(
        &binding.node,
        ast::PremKind::Let(ast::LetPrem {
            exp_l: Spanned {
                node: Noted {
                    kind: ast::ExpKind::Var(_),
                    ..
                },
                ..
            },
            exp_r: Spanned {
                node: Noted {
                    kind: ast::ExpKind::DownCast(typ, _),
                    ..
                },
                ..
            }
        }) if typ.syntax_eq(&child_typ)
    ));
}

#[test]
fn antiunification_populates_each_path_in_left_to_right_expression_order() {
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
    let groups = vec![
        vec![tuple(true, false, 1), var_exp("shared", 3)],
        vec![tuple(false, true, 5), var_exp("shared", 7)],
    ];

    let (context, template, premises) =
        antiunify::antiunify(Context::new(), groups).expect("equivalent tuple inputs");

    assert_eq!(template.len(), 2);
    let ast::ExpKind::Tuple(items) = &template[0].node.kind else {
        panic!("expected tuple template");
    };
    let template_ids = items
        .iter()
        .map(|item| match &item.node.kind {
            ast::ExpKind::Var(id) => id,
            _ => panic!("expected fresh unifier"),
        })
        .collect::<Vec<_>>();
    assert_ne!(template_ids[0].node, template_ids[1].node);
    assert!(context.frees.contains(template_ids[0]));
    assert!(context.frees.contains(template_ids[1]));
    assert!(matches!(&template[1].node.kind, ast::ExpKind::Var(id) if id.node == "shared"));

    let compared_values = |prems: &[ast::Prem]| {
        prems
            .iter()
            .map(|prem| {
                let ast::PremKind::If(if_prem) = &prem.node else {
                    panic!("expected equality premise");
                };
                let ast::ExpKind::Cmp(_, ast::OpTyp::Bool, _, exp_r) = &if_prem.exp.node.kind
                else {
                    panic!("expected equality comparison");
                };
                let ast::ExpKind::Bool(value) = exp_r.node.kind else {
                    panic!("expected original boolean expression");
                };
                value
            })
            .collect::<Vec<_>>()
    };
    assert_eq!(premises.len(), 2);
    assert_eq!(compared_values(&premises[0]), vec![true, false]);
    assert_eq!(compared_values(&premises[1]), vec![false, true]);
}

#[test]
fn antiunification_uses_runtime_equivalence_for_plain_type_aliases() {
    let alias_id = id("Flag", 1);
    let alias_typ = Spanned::new(ast::TypKind::Var(alias_id.clone(), vec![]), span(1));
    let mut context = Context::new();
    context.tdenv.insert(
        alias_id,
        TypeDef::Defined(
            vec![],
            Box::new(Spanned::new(ast::DefTypKind::Plain(typ::bool()), span(1))),
        ),
    );
    let alias_value = exp(ast::ExpKind::Bool(true), alias_typ.node, 2);
    let bool_value = exp(ast::ExpKind::Bool(false), ast::TypKind::Bool, 3);

    let (_, template, premises) =
        antiunify::antiunify(context, vec![vec![alias_value], vec![bool_value]])
            .expect("plain alias is equivalent to its underlying type");

    assert!(matches!(template[0].node.kind, ast::ExpKind::Var(_)));
    assert_eq!(
        premises.iter().map(Vec::len).collect::<Vec<_>>(),
        vec![1, 1]
    );
}

#[test]
fn analysis_preserves_rule_paths_and_populates_antiunified_inputs_in_order() {
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
    let relation_not_typ = Spanned::new(
        Mixfix::Seq(vec![
            Mixfix::Arg(Spanned::new(
                ast::TypKind::Tuple(vec![typ::bool(), typ::bool()]),
                span(1),
            )),
            Mixfix::Arg(typ::bool()),
        ]),
        span(1),
    );
    let rule = |name: &str, input: ast::Exp, output: bool, line: i64| {
        Spanned::new(
            ast::RuleKind {
                id: id(name, line),
                not_exp: Mixfix::Seq(vec![
                    Mixfix::Arg(input),
                    Mixfix::Arg(exp(ast::ExpKind::Bool(output), ast::TypKind::Bool, line)),
                ]),
                prems: vec![],
            },
            span(line),
        )
    };
    let rules = vec![
        rule("first", tuple(true, false, 2), true, 2),
        rule("second", tuple(false, true, 5), false, 5),
    ];
    let spec = vec![Spanned::new(
        ast::DefKind::Rel(ast::Rel {
            id: id("relation", 1),
            not_typ: relation_not_typ,
            input_hint: InputHint::new(vec![0]),
            rule_groups: vec![Spanned::new((id("group", 1), rules), span(1))],
            else_group: None,
            hints: vec![],
        }),
        span(1),
    )];

    let analyzed = analyze::analyze_spec(&spec).expect("analyzable relation");

    let p4spec_rust::lang::al::ast::DefKind::Rel(relation) = &analyzed[0].node else {
        panic!("expected relation definition");
    };
    let [rule_group] = relation.rule_groups.as_slice() else {
        panic!("expected one rule group");
    };
    assert_eq!(rule_group.node.id.node, "group");
    assert_eq!(
        rule_group
            .node
            .rule_paths
            .iter()
            .map(|path| path.id.node.as_str())
            .collect::<Vec<_>>(),
        vec!["first", "second"]
    );
    let ast::ExpKind::Tuple(items) = &rule_group.node.rule_match.exps_signature[0].node.kind else {
        panic!("expected tuple rule signature");
    };
    assert!(
        items
            .iter()
            .all(|item| matches!(item.node.kind, ast::ExpKind::Var(_)))
    );
    let compared_values = |prems: &[ast::Prem]| {
        prems
            .iter()
            .map(|prem| {
                let ast::PremKind::If(if_prem) = &prem.node else {
                    panic!("expected populated equality premise");
                };
                let ast::ExpKind::Cmp(_, _, _, exp_r) = &if_prem.exp.node.kind else {
                    panic!("expected equality comparison");
                };
                let ast::ExpKind::Bool(value) = exp_r.node.kind else {
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
        rule_group.node.rule_paths[0].exps_output[0].node.kind,
        ast::ExpKind::Bool(true)
    ));
    assert!(matches!(
        rule_group.node.rule_paths[1].exps_output[0].node.kind,
        ast::ExpKind::Bool(false)
    ));
}

#[test]
fn clause_analysis_orders_partial_then_repeated_then_source_premises() {
    let tuple_typ = Spanned::new(
        ast::TypKind::Tuple(vec![typ::bool(), typ::bool(), typ::bool()]),
        span(1),
    );
    let tuple = exp(
        ast::ExpKind::Tuple(vec![
            var_exp("x", 2),
            var_exp("x", 3),
            exp(ast::ExpKind::Bool(true), ast::TypKind::Bool, 4),
        ]),
        tuple_typ.node.clone(),
        2,
    );
    let clause = Spanned::new(
        ast::ClauseKind {
            args: vec![Spanned::new(ast::ArgKind::Exp(Box::new(tuple)), span(2))],
            expression: var_exp("x", 5),
            premises: vec![Spanned::new(
                ast::PremKind::Debug(ast::DebugPrem {
                    exp: exp(ast::ExpKind::Bool(false), ast::TypKind::Bool, 6),
                }),
                span(6),
            )],
        },
        span(2),
    );
    let spec = vec![Spanned::new(
        ast::DefKind::FuncDec(ast::FuncDec {
            id: id("function", 1),
            tparams: vec![],
            params: vec![Spanned::new(ast::ParamKind::Exp(tuple_typ), span(1))],
            typ: typ::bool(),
            clauses: vec![clause],
            else_clause: None,
            hints: vec![],
        }),
        span(1),
    )];

    let analyzed = analyze::analyze_spec(&spec).expect("analyzable function");

    let p4spec_rust::lang::al::ast::DefKind::FuncDec(function) = &analyzed[0].node else {
        panic!("expected function definition");
    };
    let prems = &function.clauses[0].node.premises;
    assert_eq!(prems.len(), 3);
    assert!(matches!(
        &prems[0].node,
        ast::PremKind::If(ast::IfPrem {
            exp: Spanned {
                node: Noted {
                    kind: ast::ExpKind::Cmp(_, _, _, exp_r),
                    ..
                },
                ..
            }
        }) if matches!(exp_r.node.kind, ast::ExpKind::Bool(true))
    ));
    assert!(matches!(
        &prems[1].node,
        ast::PremKind::If(ast::IfPrem {
            exp: Spanned {
                node: Noted {
                    kind: ast::ExpKind::Cmp(_, _, _, exp_r),
                    ..
                },
                ..
            }
        }) if matches!(exp_r.node.kind, ast::ExpKind::Var(_))
    ));
    assert!(matches!(&prems[2].node, ast::PremKind::Debug(_)));
}

#[test]
fn otherwise_clauses_and_rules_reject_impure_premises_at_the_branch_span() {
    let impure_premise = |line: i64| {
        Spanned::new(
            ast::PremKind::If(ast::IfPrem {
                exp: exp(ast::ExpKind::Bool(true), ast::TypKind::Bool, line),
            }),
            span(line),
        )
    };
    let else_clause = Spanned::new(
        ast::ClauseKind {
            args: vec![Spanned::new(
                ast::ArgKind::Exp(Box::new(var_exp("x", 10))),
                span(10),
            )],
            expression: var_exp("x", 10),
            premises: vec![impure_premise(11)],
        },
        span(10),
    );
    let function_spec = vec![Spanned::new(
        ast::DefKind::FuncDec(ast::FuncDec {
            id: id("function", 9),
            tparams: vec![],
            params: vec![Spanned::new(ast::ParamKind::Exp(typ::bool()), span(9))],
            typ: typ::bool(),
            clauses: vec![],
            else_clause: Some(else_clause),
            hints: vec![],
        }),
        span(9),
    )];

    let function_error =
        analyze::analyze_spec(&function_spec).expect_err("impure otherwise clause");
    assert_eq!(function_error.kind, AlgoErrorKind::ImpureElsePremises);
    assert_eq!(function_error.span, span(10));

    let relation_not_typ = Spanned::new(Mixfix::Arg(typ::bool()), span(20));
    let else_rule = Spanned::new(
        ast::RuleKind {
            id: id("else_rule", 21),
            not_exp: Mixfix::Arg(var_exp("input", 21)),
            prems: vec![impure_premise(22)],
        },
        span(21),
    );
    let relation_spec = vec![Spanned::new(
        ast::DefKind::Rel(ast::Rel {
            id: id("relation", 20),
            not_typ: relation_not_typ,
            input_hint: InputHint::new(vec![0]),
            rule_groups: vec![],
            else_group: Some(Spanned::new((id("else_group", 20), else_rule), span(20))),
            hints: vec![],
        }),
        span(20),
    )];

    let relation_error = analyze::analyze_spec(&relation_spec).expect_err("impure otherwise rule");
    assert_eq!(relation_error.kind, AlgoErrorKind::ImpureElsePremises);
    assert_eq!(relation_error.span, span(21));
}

#[test]
fn table_analysis_rejects_overlapping_and_missing_variant_patterns() {
    let choice_id = id("Choice", 1);
    let choice_typ = Spanned::new(ast::TypKind::Var(choice_id.clone(), vec![]), span(1));
    let origin = Spanned::new((choice_id.clone(), vec![]), span(1));
    let choice_def = Spanned::new(
        ast::DefKind::Typ(ast::TypDef {
            id: choice_id,
            tparams: vec![],
            def_typ: Spanned::new(
                ast::DefTypKind::Variant(vec![
                    (not_typ("A", 1), origin.clone(), vec![]),
                    (not_typ("B", 1), origin, vec![]),
                ]),
                span(1),
            ),
            hints: vec![],
        }),
        span(1),
    );
    let table = |rows: Vec<ast::TableRow>, line: i64| {
        Spanned::new(
            ast::DefKind::TableDec(ast::TableDec {
                id: id("table", line),
                params: vec![Spanned::new(
                    ast::ParamKind::Exp(choice_typ.clone()),
                    span(line),
                )],
                typ: typ::bool(),
                rows,
                hints: vec![],
            }),
            span(line - 1),
        )
    };
    let row = |pattern: ast::Exp, line: i64| {
        Spanned::new(
            (
                vec![Spanned::new(
                    ast::ArgKind::Exp(Box::new(pattern)),
                    span(line),
                )],
                exp(ast::ExpKind::Bool(true), ast::TypKind::Bool, line),
            ),
            span(line),
        )
    };
    let wildcard = |name: &str, line: i64| {
        exp(
            ast::ExpKind::Var(id(name, line)),
            choice_typ.node.clone(),
            line,
        )
    };
    let overlap_spec = vec![
        choice_def.clone(),
        table(
            vec![
                row(wildcard("left", 10), 10),
                row(wildcard("right", 11), 11),
            ],
            9,
        ),
    ];

    let overlap_error =
        analyze::analyze_spec(&overlap_spec).expect_err("overlapping wildcard rows");
    assert_eq!(overlap_error.kind, AlgoErrorKind::OverlappingTablePatterns);
    assert_eq!(overlap_error.span, span(8));

    let keyword = Spanned::new(Atom::Keyword("A".to_owned()), span(20));
    let case = exp(
        ast::ExpKind::Case(Box::new(Mixfix::Atom(keyword))),
        choice_typ.node.clone(),
        20,
    );
    let case = exp(
        ast::ExpKind::UpCast(choice_typ.clone(), Box::new(case)),
        choice_typ.node.clone(),
        20,
    );
    let missing_spec = vec![choice_def, table(vec![row(case, 20)], 19)];

    let missing_error = analyze::analyze_spec(&missing_spec).expect_err("missing B variant row");
    assert_eq!(missing_error.kind, AlgoErrorKind::MissingTablePatterns);
    assert_eq!(missing_error.span, span(18));
}

#[test]
fn analysis_preserves_definition_clause_and_table_row_order() {
    let choice_id = id("Choice", 2);
    let choice_typ = Spanned::new(ast::TypKind::Var(choice_id.clone(), vec![]), span(2));
    let origin = Spanned::new((choice_id.clone(), vec![]), span(2));
    let choice_def = Spanned::new(
        ast::DefKind::Typ(ast::TypDef {
            id: choice_id,
            tparams: vec![],
            def_typ: Spanned::new(
                ast::DefTypKind::Variant(vec![
                    (not_typ("A", 2), origin.clone(), vec![]),
                    (not_typ("B", 2), origin, vec![]),
                ]),
                span(2),
            ),
            hints: vec![],
        }),
        span(2),
    );
    let clause = |name: &str, line: i64| {
        Spanned::new(
            ast::ClauseKind {
                args: vec![Spanned::new(
                    ast::ArgKind::Exp(Box::new(var_exp(name, line))),
                    span(line),
                )],
                expression: var_exp(name, line),
                premises: vec![],
            },
            span(line),
        )
    };
    let function_def = Spanned::new(
        ast::DefKind::FuncDec(ast::FuncDec {
            id: id("function", 3),
            tparams: vec![],
            params: vec![Spanned::new(ast::ParamKind::Exp(typ::bool()), span(3))],
            typ: typ::bool(),
            clauses: vec![clause("first_clause", 4), clause("second_clause", 5)],
            else_clause: None,
            hints: vec![],
        }),
        span(3),
    );
    let row = |name: &str, value: bool, line: i64| {
        let pattern = exp(
            ast::ExpKind::Var(id(name, line)),
            choice_typ.node.clone(),
            line,
        );
        Spanned::new(
            (
                vec![Spanned::new(
                    ast::ArgKind::Exp(Box::new(pattern)),
                    span(line),
                )],
                exp(ast::ExpKind::Bool(value), ast::TypKind::Bool, line),
            ),
            span(line),
        )
    };
    let table_def = Spanned::new(
        ast::DefKind::TableDec(ast::TableDec {
            id: id("table", 6),
            params: vec![Spanned::new(
                ast::ParamKind::Exp(choice_typ.clone()),
                span(6),
            )],
            typ: typ::bool(),
            rows: vec![row("specific", true, 7), row("_closer", false, 8)],
            hints: vec![],
        }),
        span(6),
    );
    let spec = vec![
        Spanned::new(
            ast::DefKind::ExternTyp(ast::ExternTyp {
                id: id("external", 1),
                hints: vec![],
            }),
            span(1),
        ),
        choice_def,
        function_def,
        table_def,
    ];

    let analyzed = analyze::analyze_spec(&spec).expect("ordered specification");

    let definition_ids = analyzed
        .iter()
        .map(|def| match &def.node {
            p4spec_rust::lang::al::ast::DefKind::ExternTyp(def) => def.id.node.as_str(),
            p4spec_rust::lang::al::ast::DefKind::Typ(def) => def.id.node.as_str(),
            p4spec_rust::lang::al::ast::DefKind::FuncDec(def) => def.id.node.as_str(),
            p4spec_rust::lang::al::ast::DefKind::TableDec(def) => def.id.node.as_str(),
            _ => panic!("unexpected definition"),
        })
        .collect::<Vec<_>>();
    assert_eq!(
        definition_ids,
        vec!["external", "Choice", "function", "table"]
    );

    let p4spec_rust::lang::al::ast::DefKind::FuncDec(function) = &analyzed[2].node else {
        panic!("expected function definition");
    };
    let clause_ids = function
        .clauses
        .iter()
        .map(|clause| {
            let ast::ArgKind::Exp(exp) = &clause.node.args[0].node else {
                panic!("expected expression argument");
            };
            let ast::ExpKind::Var(id) = &exp.node.kind else {
                panic!("expected variable argument");
            };
            id.node.as_str()
        })
        .collect::<Vec<_>>();
    assert_eq!(clause_ids, vec!["first_clause", "second_clause"]);

    let p4spec_rust::lang::al::ast::DefKind::TableDec(table) = &analyzed[3].node else {
        panic!("expected table definition");
    };
    let row_ids = table
        .table_rows
        .iter()
        .map(|row| {
            let ast::ExpKind::Var(id) = &row.node.exps_signature[0].node.kind else {
                panic!("expected variable signature");
            };
            id.node.as_str()
        })
        .collect::<Vec<_>>();
    assert_eq!(row_ids, vec!["specific", "_closer"]);
}
