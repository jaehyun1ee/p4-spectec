use p4spec_rust::{
    domain::{
        atom::Atom,
        mixfix::Mixfix,
        source::{Position, Span, Spanned},
    },
    lang::{
        common::ds::set::IdSet,
        hints::input::InputHint,
        il::{ast, free, var},
    },
};

fn span() -> Span {
    Span::default()
}
fn named_span(name: &str) -> Span {
    Span::new(Position::new(name, 0, 0), Position::new(name, 0, 0))
}
fn id(name: &str) -> ast::Id {
    Spanned::new(name.into(), span())
}
fn id_at(name: &str, at: &str) -> ast::Id {
    Spanned::new(name.into(), named_span(at))
}
fn typ() -> ast::Typ {
    Spanned::new(ast::TypKind::Bool, span())
}
fn typ_at(at: &str) -> ast::Typ {
    Spanned::new(ast::TypKind::Bool, named_span(at))
}
fn exp(kind: ast::ExpKind) -> ast::Exp {
    ast::exp(kind, ast::TypKind::Bool, span())
}
fn variable(name: &str) -> ast::Exp {
    exp(ast::ExpKind::Var(id(name)))
}
fn atom(name: &str) -> ast::Atom {
    Spanned::new(Atom::Keyword(name.into()), span())
}
fn notexp(name: &str) -> ast::NotExp {
    Mixfix::Seq(vec![Mixfix::Arg(variable(name))])
}
fn names(names: &[&str]) -> IdSet {
    names.iter().map(|name| id(name)).collect()
}
fn prem(kind: ast::PremKind) -> ast::Prem {
    Spanned::new(kind, span())
}
fn arg(kind: ast::ArgKind) -> ast::Arg {
    Spanned::new(kind, span())
}
fn rule(head: &str, prems: Vec<ast::Prem>) -> ast::Rule {
    Spanned::new(
        ast::RuleKind {
            id: id("rule"),
            not_exp: notexp(head),
            prems,
        },
        span(),
    )
}
fn clause(arg_name: &str, body: &str, prem_name: &str) -> ast::Clause {
    Spanned::new(
        ast::ClauseKind {
            args: vec![arg(ast::ArgKind::Exp(Box::new(variable(arg_name))))],
            expression: variable(body),
            premises: vec![prem(ast::PremKind::Debug(ast::DebugPrem {
                exp: variable(prem_name),
            }))],
        },
        span(),
    )
}

#[test]
fn free_helpers_are_public_and_source_insensitive() {
    assert_eq!(IdSet::new(), names(&[]));
    assert_eq!(IdSet::from([id_at("x", "left")]), names(&["x"]));
    assert_eq!(IdSet::from([id_at("x", "right")]), names(&["x"]));
}

#[test]
fn free_expression_variants_follow_the_oracle() {
    let path = ast::path(
        ast::PathKind::Slice(
            Box::new(ast::path(ast::PathKind::Root, ast::TypKind::Bool, span())),
            Box::new(variable("path_low")),
            Box::new(variable("path_high")),
        ),
        ast::TypKind::Bool,
        span(),
    );
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
                vec![Spanned::new(
                    ast::TypKind::Var(id("type_arg"), vec![]),
                    span(),
                )],
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
        assert_eq!(free::free_exp(&expression), expected, "{case}");
    }
    assert_eq!(
        free::free_exps(&[variable("many_a"), variable("many_b")]),
        names(&["many_a", "many_b"])
    );
}

#[test]
fn free_path_argument_and_premise_variants_follow_the_oracle() {
    let root = ast::path(ast::PathKind::Root, ast::TypKind::Bool, span());
    let paths = vec![
        ("root", root.clone(), names(&[])),
        (
            "index",
            ast::path(
                ast::PathKind::Idx(Box::new(root.clone()), Box::new(variable("index"))),
                ast::TypKind::Bool,
                span(),
            ),
            names(&["index"]),
        ),
        (
            "slice",
            ast::path(
                ast::PathKind::Slice(
                    Box::new(root.clone()),
                    Box::new(variable("low")),
                    Box::new(variable("high")),
                ),
                ast::TypKind::Bool,
                span(),
            ),
            names(&["low", "high"]),
        ),
        (
            "dot",
            ast::path(
                ast::PathKind::Dot(
                    Box::new(ast::path(
                        ast::PathKind::Idx(Box::new(root), Box::new(variable("nested"))),
                        ast::TypKind::Bool,
                        span(),
                    )),
                    atom("field"),
                ),
                ast::TypKind::Bool,
                span(),
            ),
            names(&["nested"]),
        ),
    ];
    for (case, path, expected) in paths {
        assert_eq!(free::free_path(&path), expected, "path {case}");
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
        free::free_args(
            &args
                .iter()
                .map(|(_, argument, _)| argument.clone())
                .collect::<Vec<_>>()
        ),
        names(&["arg"])
    );
    for (case, argument, expected) in args {
        assert_eq!(free::free_arg(&argument), expected, "argument {case}");
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
            "let",
            prem(ast::PremKind::Let(ast::LetPrem {
                exp_l: variable("left"),
                exp_r: variable("right"),
            })),
            names(&["left", "right"]),
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
        free::free_prems(
            &prems
                .iter()
                .map(|(_, premise, _)| premise.clone())
                .collect::<Vec<_>>()
        ),
        names(&[
            "debug",
            "holds",
            "if",
            "left",
            "nested",
            "not_holds",
            "right",
            "rule"
        ])
    );
    for (case, premise, expected) in prems {
        assert_eq!(free::free_prem(&premise), expected, "premise {case}");
    }
}

#[test]
fn free_aggregates_and_definition_omissions_follow_the_oracle() {
    let rule = rule(
        "head",
        vec![prem(ast::PremKind::If(ast::IfPrem {
            exp: variable("premise"),
        }))],
    );
    assert_eq!(free::free_rule(&rule), names(&["head", "premise"]));
    assert_eq!(
        free::free_rules(std::slice::from_ref(&rule)),
        names(&["head", "premise"])
    );
    let group = Spanned::new((id("group"), vec![rule.clone()]), span());
    assert_eq!(free::free_rulegroup(&group), names(&["head", "premise"]));
    assert_eq!(
        free::free_rulegroups(std::slice::from_ref(&group)),
        names(&["head", "premise"])
    );
    let else_group = Spanned::new((id("else"), rule.clone()), span());
    assert_eq!(
        free::free_elsegroup(&else_group),
        names(&["head", "premise"])
    );
    assert_eq!(free::free_elsegroup_opt(&None), names(&[]));
    assert_eq!(
        free::free_elsegroup_opt(&Some(else_group.clone())),
        names(&["head", "premise"])
    );
    let clause = clause("argument", "body", "premise");
    assert_eq!(
        free::free_clause(&clause),
        names(&["argument", "body", "premise"])
    );
    assert_eq!(
        free::free_clauses(std::slice::from_ref(&clause)),
        names(&["argument", "body", "premise"])
    );
    assert_eq!(
        free::free_elseclause(&clause),
        names(&["argument", "body", "premise"])
    );
    assert_eq!(free::free_elseclause_opt(&None), names(&[]));
    assert_eq!(
        free::free_elseclause_opt(&Some(clause.clone())),
        names(&["argument", "body", "premise"])
    );
    let row = Spanned::new(
        (
            vec![arg(ast::ArgKind::Exp(Box::new(variable("key"))))],
            variable("value"),
        ),
        span(),
    );
    assert_eq!(free::free_tablerow(&row), names(&["key", "value"]));
    assert_eq!(
        free::free_tablerows(std::slice::from_ref(&row)),
        names(&["key", "value"])
    );
    let defs = vec![
        (
            "relation",
            Spanned::new(
                ast::DefKind::Rel(ast::Rel {
                    id: id("relation"),
                    not_typ: Spanned::new(Mixfix::Arg(typ()), span()),
                    input_hint: InputHint::new(vec![]),
                    rule_groups: vec![group],
                    else_group: Some(else_group),
                    hints: vec![],
                }),
                span(),
            ),
            names(&["head", "premise"]),
        ),
        (
            "table",
            Spanned::new(
                ast::DefKind::TableDec(ast::TableDec {
                    id: id("table"),
                    params: vec![],
                    typ: typ(),
                    rows: vec![row],
                    hints: vec![],
                }),
                span(),
            ),
            names(&["key", "value"]),
        ),
        (
            "function",
            Spanned::new(
                ast::DefKind::FuncDec(ast::FuncDec {
                    id: id("function"),
                    tparams: vec![],
                    params: vec![],
                    typ: typ(),
                    clauses: vec![clause.clone()],
                    else_clause: Some(clause),
                    hints: vec![],
                }),
                span(),
            ),
            names(&["argument", "body", "premise"]),
        ),
        (
            "extern_type",
            Spanned::new(
                ast::DefKind::ExternTyp(ast::ExternTyp {
                    id: id("ignored"),
                    hints: vec![],
                }),
                span(),
            ),
            names(&[]),
        ),
        (
            "type",
            Spanned::new(
                ast::DefKind::Typ(ast::TypDef {
                    id: id("ignored"),
                    tparams: vec![],
                    def_typ: Spanned::new(ast::DefTypKind::Plain(typ()), span()),
                    hints: vec![],
                }),
                span(),
            ),
            names(&[]),
        ),
        (
            "variable",
            Spanned::new(
                ast::DefKind::Var(ast::VarDef {
                    id: id("ignored"),
                    typ: typ(),
                    hints: vec![],
                }),
                span(),
            ),
            names(&[]),
        ),
        (
            "extern_relation",
            Spanned::new(
                ast::DefKind::ExternRel(ast::ExternRel {
                    id: id("ignored"),
                    not_typ: Spanned::new(Mixfix::Arg(typ()), span()),
                    input_hint: InputHint::new(vec![]),
                    hints: vec![],
                }),
                span(),
            ),
            names(&[]),
        ),
        (
            "extern_declaration",
            Spanned::new(
                ast::DefKind::ExternDec(ast::ExternDec {
                    id: id("ignored"),
                    tparams: vec![],
                    params: vec![],
                    typ: typ(),
                    hints: vec![],
                }),
                span(),
            ),
            names(&[]),
        ),
        (
            "builtin_declaration",
            Spanned::new(
                ast::DefKind::BuiltinDec(ast::BuiltinDec {
                    id: id("ignored"),
                    tparams: vec![],
                    params: vec![],
                    typ: typ(),
                    hints: vec![],
                }),
                span(),
            ),
            names(&[]),
        ),
    ];
    for (case, definition, expected) in defs {
        assert_eq!(free::free_def(&definition), expected, "definition {case}");
    }
}

fn assert_var(exp: &ast::Exp, expected_id: &ast::Id, expected_ty: ast::TypKind) {
    let ast::ExpKind::Var(id) = &exp.node.kind else {
        panic!("expected variable expression")
    };
    assert_eq!(id, expected_id);
    assert_eq!(exp.node.note, expected_ty);
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
fn as_exp_preserves_empty_one_and_two_level_shapes_and_spans() {
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
    let ast::ExpKind::Iter(inner, (ast::Iter::Opt, binders)) = &one_false.node.kind else {
        panic!("expected one false iteration")
    };
    assert!(binders.is_empty());
    assert_eq!(&one_false.span, &id.span);
    assert_iter_type(
        &Spanned::new(one_false.node.note.clone(), typ.span.clone()),
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
    let ast::ExpKind::Iter(inner, (ast::Iter::Opt, binders)) = &one_true.node.kind else {
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
        &Spanned::new(one_true.node.note.clone(), typ.span.clone()),
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
        let ast::ExpKind::Iter(inner, (ast::Iter::List, outer_binders)) = &two.node.kind else {
            panic!("expected outer iteration")
        };
        assert_eq!(&two.span, &id.span);
        let ast::TypKind::Iter(outer_inner_typ, ast::Iter::List) = &two.node.note else {
            panic!("expected outer iteration type")
        };
        assert_eq!(outer_inner_typ.span, id.span);
        let ast::TypKind::Iter(first_inner_typ, ast::Iter::Opt) = &outer_inner_typ.node else {
            panic!("expected nested iteration type")
        };
        assert_eq!(first_inner_typ.node, ast::TypKind::Bool);
        assert_eq!(first_inner_typ.span, id.span);
        let ast::ExpKind::Iter(base, (ast::Iter::Opt, inner_binders)) = &inner.node.kind else {
            panic!("expected inner iteration")
        };
        assert_eq!(&inner.span, &id.span);
        assert_iter_type(
            &Spanned::new(inner.node.note.clone(), typ.span.clone()),
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
