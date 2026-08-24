use p4spec_rust::{
    domain::{
        atom::Atom,
        mixfix::Mixfix,
        source::{Region, Spanned},
    },
    lang::{
        hints::input::InputHint,
        il::{ast, free, var},
    },
};

fn span() -> Region {
    Region::none()
}
fn named_span(name: &str) -> Region {
    Region::for_file(name)
}
fn id(name: &str) -> ast::Id {
    Spanned::new(name.into(), span())
}
fn id_at(name: &str, at: &str) -> ast::Id {
    Spanned::new(name.into(), named_span(at))
}
fn typ() -> ast::Typ {
    Spanned::new(ast::TypKind::BoolT, span())
}
fn typ_at(at: &str) -> ast::Typ {
    Spanned::new(ast::TypKind::BoolT, named_span(at))
}
fn exp(kind: ast::ExpKind) -> ast::Exp {
    ast::Exp::new(kind, ast::TypKind::BoolT, span())
}
fn variable(name: &str) -> ast::Exp {
    exp(ast::ExpKind::VarE(id(name)))
}
fn atom(name: &str) -> ast::Atom {
    Spanned::new(Atom::Keyword(name.into()), span())
}
fn notexp(name: &str) -> ast::NotExp {
    Mixfix::Seq(vec![Mixfix::Arg(variable(name))])
}
fn names(names: &[&str]) -> free::FreeVars {
    names.iter().map(|name| (*name).into()).collect()
}
fn prem(kind: ast::PremKind) -> ast::Prem {
    Spanned::new(kind, span())
}
fn arg(kind: ast::ArgKind) -> ast::Arg {
    Spanned::new(kind, span())
}
fn rule(head: &str, prems: Vec<ast::Prem>) -> ast::Rule {
    Spanned::new((id("rule"), notexp(head), prems), span())
}
fn clause(arg_name: &str, body: &str, prem_name: &str) -> ast::Clause {
    Spanned::new(
        (
            vec![arg(ast::ArgKind::ExpA(variable(arg_name)))],
            variable(body),
            vec![prem(ast::PremKind::DebugPr(variable(prem_name)))],
        ),
        span(),
    )
}

#[test]
fn free_helpers_are_public_and_source_insensitive() {
    assert_eq!(free::empty(), names(&[]));
    assert_eq!(free::singleton(&id_at("x", "left")), names(&["x"]));
    assert_eq!(free::singleton(&id_at("x", "right")), names(&["x"]));
}

#[test]
fn free_expression_variants_follow_the_oracle() {
    let path = ast::Path::new(
        ast::PathKind::SliceP(
            Box::new(ast::Path::new(
                ast::PathKind::RootP,
                ast::TypKind::BoolT,
                span(),
            )),
            Box::new(variable("path_low")),
            Box::new(variable("path_high")),
        ),
        ast::TypKind::BoolT,
        span(),
    );
    let cases = vec![
        ("bool", exp(ast::ExpKind::BoolE(true)), names(&[])),
        (
            "num",
            exp(ast::ExpKind::NumE(ast::Num::Nat(1.into()))),
            names(&[]),
        ),
        ("text", exp(ast::ExpKind::TextE("text".into())), names(&[])),
        ("var", variable("var"), names(&["var"])),
        (
            "unary",
            exp(ast::ExpKind::UnE(
                ast::UnOp::NotOp,
                ast::OpTyp::BoolT,
                Box::new(variable("unary")),
            )),
            names(&["unary"]),
        ),
        (
            "binary",
            exp(ast::ExpKind::BinE(
                ast::BinOp::AddOp,
                ast::OpTyp::NatT,
                Box::new(variable("left")),
                Box::new(variable("right")),
            )),
            names(&["left", "right"]),
        ),
        (
            "comparison",
            exp(ast::ExpKind::CmpE(
                ast::CmpOp::EqOp,
                ast::OpTyp::BoolT,
                Box::new(variable("left")),
                Box::new(variable("right")),
            )),
            names(&["left", "right"]),
        ),
        (
            "upcast",
            exp(ast::ExpKind::UpCastE(typ(), Box::new(variable("cast")))),
            names(&["cast"]),
        ),
        (
            "downcast",
            exp(ast::ExpKind::DownCastE(typ(), Box::new(variable("cast")))),
            names(&["cast"]),
        ),
        (
            "subtype",
            exp(ast::ExpKind::SubE(
                Box::new(variable("sub")),
                typ(),
                Box::new(ast::Subcheck::SkipSC),
            )),
            names(&["sub"]),
        ),
        (
            "match",
            exp(ast::ExpKind::MatchE(
                Box::new(variable("match")),
                ast::Pattern::OptP(ast::OptPattern::Some),
            )),
            names(&["match"]),
        ),
        (
            "tuple",
            exp(ast::ExpKind::TupleE(vec![
                variable("tuple_a"),
                variable("tuple_b"),
            ])),
            names(&["tuple_a", "tuple_b"]),
        ),
        (
            "case",
            exp(ast::ExpKind::CaseE(Box::new(notexp("case")))),
            names(&["case"]),
        ),
        (
            "struct",
            exp(ast::ExpKind::StrE(vec![(
                atom("field"),
                variable("field_value"),
            )])),
            names(&["field_value"]),
        ),
        (
            "option_some",
            exp(ast::ExpKind::OptE(Some(Box::new(variable("some"))))),
            names(&["some"]),
        ),
        ("option_none", exp(ast::ExpKind::OptE(None)), names(&[])),
        (
            "list",
            exp(ast::ExpKind::ListE(vec![
                variable("list_a"),
                variable("list_b"),
            ])),
            names(&["list_a", "list_b"]),
        ),
        (
            "cons",
            exp(ast::ExpKind::ConsE(
                Box::new(variable("head")),
                Box::new(variable("tail")),
            )),
            names(&["head", "tail"]),
        ),
        (
            "concatenate",
            exp(ast::ExpKind::CatE(
                Box::new(variable("left")),
                Box::new(variable("right")),
            )),
            names(&["left", "right"]),
        ),
        (
            "membership",
            exp(ast::ExpKind::MemE(
                Box::new(variable("element")),
                Box::new(variable("set")),
            )),
            names(&["element", "set"]),
        ),
        (
            "length",
            exp(ast::ExpKind::LenE(Box::new(variable("length")))),
            names(&["length"]),
        ),
        (
            "dot",
            exp(ast::ExpKind::DotE(
                Box::new(variable("record")),
                atom("field"),
            )),
            names(&["record"]),
        ),
        (
            "index",
            exp(ast::ExpKind::IdxE(
                Box::new(variable("base")),
                Box::new(variable("index")),
            )),
            names(&["base", "index"]),
        ),
        (
            "slice",
            exp(ast::ExpKind::SliceE(
                Box::new(variable("base")),
                Box::new(variable("low")),
                Box::new(variable("high")),
            )),
            names(&["base", "low", "high"]),
        ),
        (
            "update",
            exp(ast::ExpKind::UpdE(
                Box::new(variable("base")),
                path,
                Box::new(variable("replacement")),
            )),
            names(&["base", "path_high", "path_low", "replacement"]),
        ),
        (
            "call_omits_definition_and_type_arguments",
            exp(ast::ExpKind::CallE(
                id("definition"),
                vec![Spanned::new(
                    ast::TypKind::VarT(id("type_arg"), vec![]),
                    span(),
                )],
                vec![
                    arg(ast::ArgKind::DefA(id("definition_argument"))),
                    arg(ast::ArgKind::ExpA(variable("argument"))),
                ],
            )),
            names(&["argument"]),
        ),
        (
            "iteration_omits_binders",
            exp(ast::ExpKind::IterE(
                Box::new(variable("iterated")),
                (ast::Iter::List, vec![(id("binder"), typ(), vec![])]),
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
    let root = ast::Path::new(ast::PathKind::RootP, ast::TypKind::BoolT, span());
    let paths = vec![
        ("root", root.clone(), names(&[])),
        (
            "index",
            ast::Path::new(
                ast::PathKind::IdxP(Box::new(root.clone()), Box::new(variable("index"))),
                ast::TypKind::BoolT,
                span(),
            ),
            names(&["index"]),
        ),
        (
            "slice",
            ast::Path::new(
                ast::PathKind::SliceP(
                    Box::new(root.clone()),
                    Box::new(variable("low")),
                    Box::new(variable("high")),
                ),
                ast::TypKind::BoolT,
                span(),
            ),
            names(&["low", "high"]),
        ),
        (
            "dot",
            ast::Path::new(
                ast::PathKind::DotP(
                    Box::new(ast::Path::new(
                        ast::PathKind::IdxP(Box::new(root), Box::new(variable("nested"))),
                        ast::TypKind::BoolT,
                        span(),
                    )),
                    atom("field"),
                ),
                ast::TypKind::BoolT,
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
            arg(ast::ArgKind::ExpA(variable("arg"))),
            names(&["arg"]),
        ),
        (
            "definition",
            arg(ast::ArgKind::DefA(id("definition"))),
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
    let nested = prem(ast::PremKind::IfPr(variable("nested")));
    let prems = vec![
        (
            "rule",
            prem(ast::PremKind::RulePr(
                id("relation"),
                notexp("rule"),
                InputHint::new(vec![0]),
            )),
            names(&["rule"]),
        ),
        (
            "if",
            prem(ast::PremKind::IfPr(variable("if"))),
            names(&["if"]),
        ),
        (
            "if_holds",
            prem(ast::PremKind::IfHoldPr(id("relation"), notexp("holds"))),
            names(&["holds"]),
        ),
        (
            "if_not_holds",
            prem(ast::PremKind::IfNotHoldPr(
                id("relation"),
                notexp("not_holds"),
            )),
            names(&["not_holds"]),
        ),
        (
            "let",
            prem(ast::PremKind::LetPr(variable("left"), variable("right"))),
            names(&["left", "right"]),
        ),
        (
            "iteration_omits_binder_variables",
            prem(ast::PremKind::IterPr(
                Box::new(nested),
                (
                    ast::Iter::List,
                    vec![(id("input"), typ(), vec![])],
                    vec![(id("output"), typ(), vec![])],
                ),
            )),
            names(&["nested"]),
        ),
        (
            "debug",
            prem(ast::PremKind::DebugPr(variable("debug"))),
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
    let rule = rule("head", vec![prem(ast::PremKind::IfPr(variable("premise")))]);
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
            vec![arg(ast::ArgKind::ExpA(variable("key")))],
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
                ast::DefKind::RelD(
                    id("relation"),
                    Spanned::new(Mixfix::Arg(typ()), span()),
                    InputHint::new(vec![]),
                    vec![group],
                    Some(else_group),
                    vec![],
                ),
                span(),
            ),
            names(&["head", "premise"]),
        ),
        (
            "table",
            Spanned::new(
                ast::DefKind::TableDecD(id("table"), vec![], typ(), vec![row], vec![]),
                span(),
            ),
            names(&["key", "value"]),
        ),
        (
            "function",
            Spanned::new(
                ast::DefKind::FuncDecD(
                    id("function"),
                    vec![],
                    vec![],
                    typ(),
                    vec![clause.clone()],
                    Some(clause),
                    vec![],
                ),
                span(),
            ),
            names(&["argument", "body", "premise"]),
        ),
        (
            "extern_type",
            Spanned::new(ast::DefKind::ExternTypD(id("ignored"), vec![]), span()),
            names(&[]),
        ),
        (
            "type",
            Spanned::new(
                ast::DefKind::TypD(
                    id("ignored"),
                    vec![],
                    Spanned::new(ast::DefTypKind::PlainT(typ()), span()),
                    vec![],
                ),
                span(),
            ),
            names(&[]),
        ),
        (
            "variable",
            Spanned::new(ast::DefKind::VarD(id("ignored"), typ(), vec![]), span()),
            names(&[]),
        ),
        (
            "extern_relation",
            Spanned::new(
                ast::DefKind::ExternRelD(
                    id("ignored"),
                    Spanned::new(Mixfix::Arg(typ()), span()),
                    InputHint::new(vec![]),
                    vec![],
                ),
                span(),
            ),
            names(&[]),
        ),
        (
            "extern_declaration",
            Spanned::new(
                ast::DefKind::ExternDecD(id("ignored"), vec![], vec![], typ(), vec![]),
                span(),
            ),
            names(&[]),
        ),
        (
            "builtin_declaration",
            Spanned::new(
                ast::DefKind::BuiltinDecD(id("ignored"), vec![], vec![], typ(), vec![]),
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
    let ast::ExpKind::VarE(id) = &exp.kind else {
        panic!("expected variable expression")
    };
    assert_eq!(id, expected_id);
    assert_eq!(exp.ty, expected_ty);
    assert_eq!(exp.span, expected_id.span);
}
fn assert_iter_type(typ: &ast::Typ, iter: ast::Iter, inner: ast::TypKind, inner_span: &Region) {
    let ast::TypKind::IterT(inner_typ, actual_iter) = &typ.node else {
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
        let empty = var::as_exp(&(id.clone(), typ.clone(), vec![]), dim);
        assert_var(&empty, &id, ast::TypKind::BoolT);
    }
    let one_false = var::as_exp(&(id.clone(), typ.clone(), vec![ast::Iter::Opt]), false);
    let ast::Exp {
        kind: ast::ExpKind::IterE(inner, (ast::Iter::Opt, binders)),
        ty,
        span,
    } = &one_false
    else {
        panic!("expected one false iteration")
    };
    assert!(binders.is_empty());
    assert_eq!(span, &id.span);
    assert_iter_type(
        &Spanned::new(ty.clone(), typ.span.clone()),
        ast::Iter::Opt,
        ast::TypKind::BoolT,
        &id.span,
    );
    assert_var(inner, &id, ast::TypKind::BoolT);
    let one_true = var::as_exp(&(id.clone(), typ.clone(), vec![ast::Iter::Opt]), true);
    let ast::Exp {
        kind: ast::ExpKind::IterE(inner, (ast::Iter::Opt, binders)),
        ty,
        span,
    } = &one_true
    else {
        panic!("expected one true iteration")
    };
    let [(binder_id, binder_typ, prior)] = binders.as_slice() else {
        panic!("expected one binder")
    };
    assert_eq!(binder_id, &id);
    assert_eq!(binder_typ.span, typ.span);
    assert!(prior.is_empty());
    assert_iter_type(binder_typ, ast::Iter::Opt, ast::TypKind::BoolT, &id.span);
    assert_eq!(span, &id.span);
    assert_iter_type(
        &Spanned::new(ty.clone(), typ.span.clone()),
        ast::Iter::Opt,
        ast::TypKind::BoolT,
        &id.span,
    );
    assert_var(inner, &id, ast::TypKind::BoolT);
    for dim in [false, true] {
        let two = var::as_exp(
            &(
                id.clone(),
                typ.clone(),
                vec![ast::Iter::Opt, ast::Iter::List],
            ),
            dim,
        );
        let ast::Exp {
            kind: ast::ExpKind::IterE(inner, (ast::Iter::List, outer_binders)),
            ty,
            span,
        } = &two
        else {
            panic!("expected outer iteration")
        };
        assert_eq!(span, &id.span);
        let ast::TypKind::IterT(outer_inner_typ, ast::Iter::List) = ty else {
            panic!("expected outer iteration type")
        };
        assert_eq!(outer_inner_typ.span, id.span);
        let ast::TypKind::IterT(first_inner_typ, ast::Iter::Opt) = &outer_inner_typ.node else {
            panic!("expected nested iteration type")
        };
        assert_eq!(first_inner_typ.node, ast::TypKind::BoolT);
        assert_eq!(first_inner_typ.span, id.span);
        let ast::Exp {
            kind: ast::ExpKind::IterE(base, (ast::Iter::Opt, inner_binders)),
            ty: inner_ty,
            span: inner_span,
        } = inner.as_ref()
        else {
            panic!("expected inner iteration")
        };
        assert_eq!(inner_span, &id.span);
        assert_iter_type(
            &Spanned::new(inner_ty.clone(), typ.span.clone()),
            ast::Iter::Opt,
            ast::TypKind::BoolT,
            &id.span,
        );
        assert_var(base, &id, ast::TypKind::BoolT);
        match (dim, inner_binders.as_slice(), outer_binders.as_slice()) {
            (false, [], []) => {}
            (true, [(inner_id, inner_typ, inner_prior)], [(outer_id, outer_typ, outer_prior)]) => {
                assert_eq!(inner_id, &id);
                assert_eq!(inner_typ.span, typ.span);
                assert!(inner_prior.is_empty());
                assert_iter_type(inner_typ, ast::Iter::Opt, ast::TypKind::BoolT, &id.span);
                assert_eq!(outer_id, &id);
                assert_eq!(outer_typ.span, typ.span);
                assert_eq!(outer_prior, &vec![ast::Iter::Opt]);
                let ast::TypKind::IterT(outer_inner, ast::Iter::List) = &outer_typ.node else {
                    panic!("expected outer binder type")
                };
                assert_eq!(outer_inner.span, id.span);
                let ast::TypKind::IterT(first_inner, ast::Iter::Opt) = &outer_inner.node else {
                    panic!("expected nested outer binder type")
                };
                assert_eq!(first_inner.node, ast::TypKind::BoolT);
                assert_eq!(first_inner.span, id.span);
            }
            _ => panic!("unexpected binders for dim={dim}"),
        }
    }
}
