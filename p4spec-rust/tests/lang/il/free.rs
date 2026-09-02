use p4spec_rust::{
    lang::common::source::{Position, Span},
    lang::{
        common::{
            ds::set::IdSet,
            notation::{atom::Atom, mixfix::Mixfix},
        },
        hints::input::InputHint,
        il::{ast, var},
        traits::free::Free,
    },
};

fn span() -> Span {
    Span::default()
}
fn named_span(name: &str) -> Span {
    Span::new(Position::new(name, 0, 0), Position::new(name, 0, 0))
}
fn id(name: &str) -> ast::Id {
    p4spec_rust::phrase! {
        node: name.into(),
        span: span(),
    }
}
fn id_at(name: &str, at: &str) -> ast::Id {
    p4spec_rust::phrase! {
        node: name.into(),
        span: named_span(at),
    }
}
fn typ() -> ast::Typ {
    p4spec_rust::phrase! {
        node: ast::TypKind::Bool,
        span: span(),
    }
}
fn typ_at(at: &str) -> ast::Typ {
    p4spec_rust::phrase! {
        node: ast::TypKind::Bool,
        span: named_span(at),
    }
}
fn exp(kind: ast::ExpKind) -> ast::Exp {
    p4spec_rust::note_phrase! {
        node: kind,
        note: ast::TypKind::Bool,
        span: span(),
    }
}
fn variable(name: &str) -> ast::Exp {
    exp(ast::ExpKind::Var(id(name)))
}
fn atom(name: &str) -> ast::Atom {
    p4spec_rust::phrase! {
        node: Atom::Keyword(name.into()),
        span: span(),
    }
}
fn notexp(name: &str) -> ast::NotExp {
    Mixfix::Seq(vec![Mixfix::Arg(variable(name))])
}
fn names(names: &[&str]) -> IdSet {
    names.iter().map(|name| id(name)).collect()
}
fn prem(kind: ast::PremKind) -> ast::Prem {
    p4spec_rust::phrase! {
        node: kind,
        span: span(),
    }
}
fn arg(kind: ast::ArgKind) -> ast::Arg {
    p4spec_rust::phrase! {
        node: kind,
        span: span(),
    }
}
fn rule(head: &str, prems: Vec<ast::Prem>) -> ast::Rule {
    p4spec_rust::phrase! { node: ast::RuleKind {
        id: id("rule"),
        not_exp: notexp(head),
        prems,
    }, span: span() }
}
fn clause(arg_name: &str, body: &str, prem_name: &str) -> ast::Clause {
    p4spec_rust::phrase! { node: ast::ClauseKind {
        args: vec![arg(ast::ArgKind::Exp(Box::new(variable(arg_name))))],
        expression: variable(body),
        premises: vec![prem(ast::PremKind::Debug(ast::DebugPrem {
            exp: variable(prem_name),
        }))],
    }, span: span() }
}

#[test]
fn test_free_helpers_are_public_and_source_insensitive() {
    assert_eq!(IdSet::new(), names(&[]));
    assert_eq!(IdSet::from([id_at("x", "left")]), names(&["x"]));
    assert_eq!(IdSet::from([id_at("x", "right")]), names(&["x"]));
}

#[test]
fn test_free_expression_variants_follow_the_oracle() {
    let path = p4spec_rust::note_phrase! { node: ast::PathKind::Slice(
    Box::new(p4spec_rust::note_phrase! {
        node: ast::PathKind::Root,
        note: ast::TypKind::Bool,
        span: span(),
    }),
    Box::new(variable("path_low")),
    Box::new(variable("path_high")),
    ), note: ast::TypKind::Bool, span: span() };
    let cases = vec![
        ("bool", exp(ast::ExpKind::Bool(true)), names(&[])),
        (
            "num",
            exp(ast::ExpKind::Num(ast::Num::Nat(1.into()))),
            names(&[]),
        ),
        ("text", exp(ast::ExpKind::Text("text".into())), names(&[])),
        ("var", variable("var"), names(&["var"])),
        (
            "unary",
            exp(ast::ExpKind::Un(
                ast::UnOp::Bool(p4spec_rust::lang::xl::bool::UnOp::Not),
                ast::OpTyp::Bool,
                Box::new(variable("unary")),
            )),
            names(&["unary"]),
        ),
        (
            "binary",
            exp(ast::ExpKind::Bin(
                ast::BinOp::Num(p4spec_rust::lang::xl::num::BinOp::Add),
                ast::OpTyp::Nat,
                Box::new(variable("left")),
                Box::new(variable("right")),
            )),
            names(&["left", "right"]),
        ),
        (
            "comparison",
            exp(ast::ExpKind::Cmp(
                ast::CmpOp::Bool(p4spec_rust::lang::xl::bool::CmpOp::Eq),
                ast::OpTyp::Bool,
                Box::new(variable("left")),
                Box::new(variable("right")),
            )),
            names(&["left", "right"]),
        ),
        (
            "upcast",
            exp(ast::ExpKind::UpCast(typ(), Box::new(variable("cast")))),
            names(&["cast"]),
        ),
        (
            "downcast",
            exp(ast::ExpKind::DownCast(typ(), Box::new(variable("cast")))),
            names(&["cast"]),
        ),
        (
            "subtype",
            exp(ast::ExpKind::Sub(
                Box::new(variable("sub")),
                typ(),
                Box::new(ast::Subcheck::Skip),
            )),
            names(&["sub"]),
        ),
        (
            "match",
            exp(ast::ExpKind::Match(
                Box::new(variable("match")),
                ast::Pattern::Opt(ast::OptPattern::Some),
            )),
            names(&["match"]),
        ),
        (
            "tuple",
            exp(ast::ExpKind::Tuple(vec![
                variable("tuple_a"),
                variable("tuple_b"),
            ])),
            names(&["tuple_a", "tuple_b"]),
        ),
        (
            "case",
            exp(ast::ExpKind::Case(Box::new(notexp("case")))),
            names(&["case"]),
        ),
        (
            "struct",
            exp(ast::ExpKind::Str(vec![(
                atom("field"),
                variable("field_value"),
            )])),
            names(&["field_value"]),
        ),
        (
            "option_some",
            exp(ast::ExpKind::Opt(Some(Box::new(variable("some"))))),
            names(&["some"]),
        ),
        ("option_none", exp(ast::ExpKind::Opt(None)), names(&[])),
        (
            "list",
            exp(ast::ExpKind::List(vec![
                variable("list_a"),
                variable("list_b"),
            ])),
            names(&["list_a", "list_b"]),
        ),
        (
            "cons",
            exp(ast::ExpKind::Cons(
                Box::new(variable("head")),
                Box::new(variable("tail")),
            )),
            names(&["head", "tail"]),
        ),
        (
            "concatenate",
            exp(ast::ExpKind::Cat(
                Box::new(variable("left")),
                Box::new(variable("right")),
            )),
            names(&["left", "right"]),
        ),
        (
            "membership",
            exp(ast::ExpKind::Mem(
                Box::new(variable("element")),
                Box::new(variable("set")),
            )),
            names(&["element", "set"]),
        ),
        (
            "length",
            exp(ast::ExpKind::Len(Box::new(variable("length")))),
            names(&["length"]),
        ),
        (
            "dot",
            exp(ast::ExpKind::Dot(
                Box::new(variable("record")),
                atom("field"),
            )),
            names(&["record"]),
        ),
        (
            "index",
            exp(ast::ExpKind::Idx(
                Box::new(variable("base")),
                Box::new(variable("index")),
            )),
            names(&["base", "index"]),
        ),
        (
            "slice",
            exp(ast::ExpKind::Slice(
                Box::new(variable("base")),
                Box::new(variable("low")),
                Box::new(variable("high")),
            )),
            names(&["base", "low", "high"]),
        ),
        (
            "update",
            exp(ast::ExpKind::Upd(
                Box::new(variable("base")),
                path,
                Box::new(variable("replacement")),
            )),
            names(&["base", "path_high", "path_low", "replacement"]),
        ),
        (
            "call_omits_definition_and_type_arguments",
            exp(ast::ExpKind::Call(
                id("definition"),
                vec![p4spec_rust::phrase! {
                    node: ast::TypKind::Var(id("type_arg"), vec![]),
                    span: span(),
                }],
                vec![
                    arg(ast::ArgKind::Def(id("definition_argument"))),
                    arg(ast::ArgKind::Exp(Box::new(variable("argument")))),
                ],
            )),
            names(&["argument"]),
        ),
        (
            "iteration_omits_binders",
            exp(ast::ExpKind::Iter(
                Box::new(variable("iterated")),
                (
                    ast::Iter::List,
                    vec![ast::Var {
                        id: id("binder"),
                        typ: typ(),
                        iters: vec![],
                    }],
                ),
            )),
            names(&["iterated"]),
        ),
    ];
    for (case, expression, expected) in cases {
        assert_eq!(expression.free(), expected, "{case}");
    }
    assert_eq!(
        [variable("many_a"), variable("many_b")].free(),
        names(&["many_a", "many_b"])
    );
}

#[test]
fn test_free_path_argument_and_premise_variants_follow_the_oracle() {
    let root = p4spec_rust::note_phrase! {
        node: ast::PathKind::Root,
        note: ast::TypKind::Bool,
        span: span(),
    };
    let paths = vec![
        ("root", root.clone(), names(&[])),
        (
            "index",
            p4spec_rust::note_phrase! { node: ast::PathKind::Idx(
                Box::new(root.clone()),
                Box::new(variable("index")),
            ), note: ast::TypKind::Bool, span: span() },
            names(&["index"]),
        ),
        (
            "slice",
            p4spec_rust::note_phrase! { node: ast::PathKind::Slice(
            Box::new(root.clone()),
            Box::new(variable("low")),
            Box::new(variable("high")),
            ), note: ast::TypKind::Bool, span: span() },
            names(&["low", "high"]),
        ),
        (
            "dot",
            p4spec_rust::note_phrase! { node: ast::PathKind::Dot(
            Box::new(p4spec_rust::note_phrase! {
                node: ast::PathKind::Idx(
                    Box::new(root),
                    Box::new(variable("nested")),
                ),
                note: ast::TypKind::Bool,
                span: span(),
            }),
            atom("field"),
            ), note: ast::TypKind::Bool, span: span() },
            names(&["nested"]),
        ),
    ];
    for (case, path, expected) in paths {
        assert_eq!(path.free(), expected, "path {case}");
    }
    let args = vec![
        (
            "expression",
            arg(ast::ArgKind::Exp(Box::new(variable("arg")))),
            names(&["arg"]),
        ),
        (
            "definition",
            arg(ast::ArgKind::Def(id("definition"))),
            names(&[]),
        ),
    ];
    assert_eq!(
        args.iter()
            .map(|(_, argument, _)| argument.clone())
            .collect::<Vec<_>>()
            .free(),
        names(&["arg"])
    );
    for (case, argument, expected) in args {
        assert_eq!(argument.free(), expected, "argument {case}");
    }
    let nested = prem(ast::PremKind::If(ast::IfPrem {
        exp: variable("nested"),
    }));
    let prems = vec![
        (
            "rule",
            prem(ast::PremKind::Rule(ast::RulePrem {
                id: id("relation"),
                not_exp: notexp("rule"),
                input_hint: InputHint::new(vec![0]),
            })),
            names(&["rule"]),
        ),
        (
            "if",
            prem(ast::PremKind::If(ast::IfPrem {
                exp: variable("if"),
            })),
            names(&["if"]),
        ),
        (
            "if_holds",
            prem(ast::PremKind::IfHold(ast::IfHoldPrem {
                id: id("relation"),
                not_exp: notexp("holds"),
            })),
            names(&["holds"]),
        ),
        (
            "if_not_holds",
            prem(ast::PremKind::IfNotHold(ast::IfNotHoldPrem {
                id: id("relation"),
                not_exp: notexp("not_holds"),
            })),
            names(&["not_holds"]),
        ),
        (
            "iteration_omits_binder_variables",
            prem(ast::PremKind::Iter(ast::IteratedPrem {
                prem: Box::new(nested),
                iter_prem: ast::IterPrem {
                    iter: ast::Iter::List,
                    vars_bound: vec![ast::Var {
                        id: id("input"),
                        typ: typ(),
                        iters: vec![],
                    }],
                    vars_bind: vec![ast::Var {
                        id: id("output"),
                        typ: typ(),
                        iters: vec![],
                    }],
                },
            })),
            names(&["nested"]),
        ),
        (
            "debug",
            prem(ast::PremKind::Debug(ast::DebugPrem {
                exp: variable("debug"),
            })),
            names(&["debug"]),
        ),
    ];
    assert_eq!(
        prems
            .iter()
            .map(|(_, premise, _)| premise.clone())
            .collect::<Vec<_>>()
            .free(),
        names(&["debug", "holds", "if", "nested", "not_holds", "rule"])
    );
    for (case, premise, expected) in prems {
        assert_eq!(premise.free(), expected, "premise {case}");
    }
}

#[test]
fn test_free_aggregates_and_definition_omissions_follow_the_oracle() {
    let rule = rule(
        "head",
        vec![prem(ast::PremKind::If(ast::IfPrem {
            exp: variable("premise"),
        }))],
    );
    assert_eq!(rule.free(), names(&["head", "premise"]));
    assert_eq!(
        std::slice::from_ref(&rule).free(),
        names(&["head", "premise"])
    );
    let group = p4spec_rust::phrase! {
        node: (id("group"), vec![rule.clone()]),
        span: span(),
    };
    assert_eq!(group.free(), names(&["head", "premise"]));
    assert_eq!(
        std::slice::from_ref(&group).free(),
        names(&["head", "premise"])
    );
    let else_group = p4spec_rust::phrase! {
        node: (id("else"), rule.clone()),
        span: span(),
    };
    assert_eq!(else_group.free(), names(&["head", "premise"]));
    assert_eq!(Option::<ast::ElseGroup>::None.free(), names(&[]));
    assert_eq!(Some(else_group.clone()).free(), names(&["head", "premise"]));
    let clause = clause("argument", "body", "premise");
    assert_eq!(clause.free(), names(&["argument", "body", "premise"]));
    assert_eq!(
        std::slice::from_ref(&clause).free(),
        names(&["argument", "body", "premise"])
    );
    assert_eq!(clause.free(), names(&["argument", "body", "premise"]));
    assert_eq!(Option::<ast::ElseClause>::None.free(), names(&[]));
    assert_eq!(
        Some(clause.clone()).free(),
        names(&["argument", "body", "premise"])
    );
    let row = p4spec_rust::phrase! { node: (
        vec![arg(ast::ArgKind::Exp(Box::new(variable("key"))))],
        variable("value"),
    ), span: span() };
    assert_eq!(row.free(), names(&["key", "value"]));
    assert_eq!(std::slice::from_ref(&row).free(), names(&["key", "value"]));
    let defs = vec![
        (
            "relation",
            p4spec_rust::phrase! { node: ast::DefKind::Rel(ast::Rel {
                id: id("relation"),
                not_typ: p4spec_rust::phrase! {
                    node: Mixfix::Arg(typ()),
                    span: span(),
                },
                input_hint: InputHint::new(vec![]),
                rule_groups: vec![group],
                else_group: Some(else_group),
                hints: vec![],
            }), span: span() },
            names(&["head", "premise"]),
        ),
        (
            "table",
            p4spec_rust::phrase! { node: ast::DefKind::TableDec(ast::TableDec {
                id: id("table"),
                params: vec![],
                typ: typ(),
                rows: vec![row],
                hints: vec![],
            }), span: span() },
            names(&["key", "value"]),
        ),
        (
            "function",
            p4spec_rust::phrase! { node: ast::DefKind::FuncDec(ast::FuncDec {
                id: id("function"),
                tparams: vec![],
                params: vec![],
                typ: typ(),
                clauses: vec![clause.clone()],
                else_clause: Some(clause),
                hints: vec![],
            }), span: span() },
            names(&["argument", "body", "premise"]),
        ),
        (
            "extern_type",
            p4spec_rust::phrase! { node: ast::DefKind::ExternTyp(ast::ExternTyp {
                id: id("ignored"),
                hints: vec![],
            }), span: span() },
            names(&[]),
        ),
        (
            "type",
            p4spec_rust::phrase! { node: ast::DefKind::Typ(ast::TypDef {
                id: id("ignored"),
                tparams: vec![],
                def_typ: p4spec_rust::phrase! {
                    node: ast::DefTypKind::Plain(typ()),
                    span: span(),
                },
                hints: vec![],
            }), span: span() },
            names(&[]),
        ),
        (
            "variable",
            p4spec_rust::phrase! { node: ast::DefKind::Var(ast::VarDef {
                id: id("ignored"),
                typ: typ(),
                hints: vec![],
            }), span: span() },
            names(&[]),
        ),
        (
            "extern_relation",
            p4spec_rust::phrase! { node: ast::DefKind::ExternRel(ast::ExternRel {
                id: id("ignored"),
                not_typ: p4spec_rust::phrase! {
                    node: Mixfix::Arg(typ()),
                    span: span(),
                },
                input_hint: InputHint::new(vec![]),
                hints: vec![],
            }), span: span() },
            names(&[]),
        ),
        (
            "extern_declaration",
            p4spec_rust::phrase! { node: ast::DefKind::ExternDec(ast::ExternDec {
                id: id("ignored"),
                tparams: vec![],
                params: vec![],
                typ: typ(),
                hints: vec![],
            }), span: span() },
            names(&[]),
        ),
        (
            "builtin_declaration",
            p4spec_rust::phrase! { node: ast::DefKind::BuiltinDec(ast::BuiltinDec {
                id: id("ignored"),
                tparams: vec![],
                params: vec![],
                typ: typ(),
                hints: vec![],
            }), span: span() },
            names(&[]),
        ),
    ];
    for (case, definition, expected) in defs {
        assert_eq!(definition.free(), expected, "definition {case}");
    }
}

fn assert_var(exp: &ast::Exp, expected_id: &ast::Id, expected_ty: ast::TypKind) {
    let ast::ExpKind::Var(id) = &exp.node else {
        panic!("expected variable expression")
    };
    assert_eq!(id, expected_id);
    assert_eq!(exp.note, expected_ty);
    assert_eq!(exp.span, expected_id.span);
}
fn assert_iter_type(typ: &ast::Typ, iter: ast::Iter, inner: ast::TypKind, inner_span: &Span) {
    let ast::TypKind::Iter(inner_typ, actual_iter) = &typ.node else {
        panic!("expected iteration type")
    };
    assert_eq!(*actual_iter, iter);
    assert_eq!(inner_typ.node, inner);
    assert_eq!(&inner_typ.span, inner_span);
}

#[test]
fn test_as_exp_preserves_empty_one_and_two_level_shapes_and_spans() {
    let id = id_at("x", "identifier");
    let typ = typ_at("type");
    for dim in [false, true] {
        let empty = var::as_exp(
            dim,
            &ast::Var {
                id: id.clone(),
                typ: typ.clone(),
                iters: vec![],
            },
        );
        assert_var(&empty, &id, ast::TypKind::Bool);
    }
    let one_false = var::as_exp(
        false,
        &ast::Var {
            id: id.clone(),
            typ: typ.clone(),
            iters: vec![ast::Iter::Opt],
        },
    );
    let ast::ExpKind::Iter(inner, (ast::Iter::Opt, binders)) = &one_false.node else {
        panic!("expected one false iteration")
    };
    assert!(binders.is_empty());
    assert_eq!(&one_false.span, &id.span);
    assert_iter_type(
        &p4spec_rust::phrase! {
            node: one_false.note.clone(),
            span: typ.span.clone(),
        },
        ast::Iter::Opt,
        ast::TypKind::Bool,
        &id.span,
    );
    assert_var(inner, &id, ast::TypKind::Bool);
    let one_true = var::as_exp(
        true,
        &ast::Var {
            id: id.clone(),
            typ: typ.clone(),
            iters: vec![ast::Iter::Opt],
        },
    );
    let ast::ExpKind::Iter(inner, (ast::Iter::Opt, binders)) = &one_true.node else {
        panic!("expected one true iteration")
    };
    let [binder] = binders.as_slice() else {
        panic!("expected one binder")
    };
    assert_eq!(&binder.id, &id);
    assert_eq!(binder.typ.span, typ.span);
    assert!(binder.iters.is_empty());
    assert_iter_type(&binder.typ, ast::Iter::Opt, ast::TypKind::Bool, &id.span);
    assert_eq!(&one_true.span, &id.span);
    assert_iter_type(
        &p4spec_rust::phrase! {
            node: one_true.note.clone(),
            span: typ.span.clone(),
        },
        ast::Iter::Opt,
        ast::TypKind::Bool,
        &id.span,
    );
    assert_var(inner, &id, ast::TypKind::Bool);
    for dim in [false, true] {
        let two = var::as_exp(
            dim,
            &ast::Var {
                id: id.clone(),
                typ: typ.clone(),
                iters: vec![ast::Iter::Opt, ast::Iter::List],
            },
        );
        let ast::ExpKind::Iter(inner, (ast::Iter::List, outer_binders)) = &two.node else {
            panic!("expected outer iteration")
        };
        assert_eq!(&two.span, &id.span);
        let ast::TypKind::Iter(outer_inner_typ, ast::Iter::List) = &two.note else {
            panic!("expected outer iteration type")
        };
        assert_eq!(outer_inner_typ.span, id.span);
        let ast::TypKind::Iter(first_inner_typ, ast::Iter::Opt) = &outer_inner_typ.node else {
            panic!("expected nested iteration type")
        };
        assert_eq!(first_inner_typ.node, ast::TypKind::Bool);
        assert_eq!(first_inner_typ.span, id.span);
        let ast::ExpKind::Iter(base, (ast::Iter::Opt, inner_binders)) = &inner.node else {
            panic!("expected inner iteration")
        };
        assert_eq!(&inner.span, &id.span);
        assert_iter_type(
            &p4spec_rust::phrase! {
                node: inner.note.clone(),
                span: typ.span.clone(),
            },
            ast::Iter::Opt,
            ast::TypKind::Bool,
            &id.span,
        );
        assert_var(base, &id, ast::TypKind::Bool);
        match (dim, inner_binders.as_slice(), outer_binders.as_slice()) {
            (false, [], []) => {}
            (true, [inner], [outer]) => {
                assert_eq!(&inner.id, &id);
                assert_eq!(inner.typ.span, typ.span);
                assert!(inner.iters.is_empty());
                assert_iter_type(&inner.typ, ast::Iter::Opt, ast::TypKind::Bool, &id.span);
                assert_eq!(&outer.id, &id);
                assert_eq!(outer.typ.span, typ.span);
                assert_eq!(outer.iters, vec![ast::Iter::Opt]);
                let ast::TypKind::Iter(outer_inner, ast::Iter::List) = &outer.typ.node else {
                    panic!("expected outer binder type")
                };
                assert_eq!(outer_inner.span, id.span);
                let ast::TypKind::Iter(first_inner, ast::Iter::Opt) = &outer_inner.node else {
                    panic!("expected nested outer binder type")
                };
                assert_eq!(first_inner.node, ast::TypKind::Bool);
                assert_eq!(first_inner.span, id.span);
            }
            _ => panic!("unexpected binders for dim={dim}"),
        }
    }
}
