use p4spec_rust::{
    lang::common::source::{Position, Span, Spanned},
    lang::{
        common::{
            ds::{map::IdMap, set::IdSet},
            notation::mixfix::Mixfix,
            noted::Noted,
        },
        hints::input::InputHint,
        il::{ast, fresh},
        traits::print::Print,
    },
};

fn typ() -> ast::Typ {
    Spanned::new(ast::TypKind::Bool, Span::default())
}
fn id(name: &str) -> ast::Id {
    Spanned::new(name.into(), Span::default())
}
fn exp(kind: ast::ExpKind) -> ast::Exp {
    p4spec_rust::spanned! {
        node: Noted {
            kind,
            note: ast::TypKind::Bool,
        },
        span: Span::default(),
    }
}
fn var(name: &str) -> ast::Exp {
    exp(ast::ExpKind::Var(id(name)))
}
fn arg(kind: ast::ArgKind) -> ast::Arg {
    Spanned::new(kind, Span::default())
}
fn prem(kind: ast::PremKind) -> ast::Prem {
    Spanned::new(kind, Span::default())
}
fn notexp(name: &str) -> ast::NotExp {
    Mixfix::Seq(vec![Mixfix::Arg(var(name))])
}
fn not_typ() -> ast::NotTyp {
    Spanned::new(Mixfix::Arg(typ()), Span::default())
}
fn names(names: &[&str]) -> IdSet {
    names.iter().map(|name| id(name)).collect()
}
fn hint() -> ast::Hint {
    (
        Spanned::new("meta".into(), Span::default()),
        Spanned::new(
            p4spec_rust::lang::el::ast::ExpKind::Var(Spanned::new(
                "payload".into(),
                Span::default(),
            )),
            Span::default(),
        ),
    )
}

#[test]
fn printer_renders_nested_premises_and_definition_spec_goldens() {
    let iteration = ast::IterPrem {
        iter: ast::Iter::List,
        vars_bound: vec![ast::Var {
            id: id("bound"),
            typ: typ(),
            iters: vec![],
        }],
        vars_bind: vec![ast::Var {
            id: id("output"),
            typ: typ(),
            iters: vec![ast::Iter::Opt],
        }],
    };
    let nested = prem(ast::PremKind::Iter(ast::IteratedPrem {
        prem: Box::new(prem(ast::PremKind::Iter(ast::IteratedPrem {
            prem: Box::new(prem(ast::PremKind::If(ast::IfPrem { exp: var("ready") }))),
            iter_prem: iteration.clone(),
        }))),
        iter_prem: iteration,
    }));
    assert_eq!(
        Print::to_string(&nested),
        "(if ready)*{bound <- bound*, output? -> output?*}*{bound <- bound*, output? -> output?*}"
    );
    let rule = Spanned::new(
        ast::RuleKind {
            id: id("r"),
            not_exp: notexp("head"),
            prems: vec![
                prem(ast::PremKind::Rule(ast::RulePrem {
                    id: id("relation"),
                    not_exp: notexp("input"),
                    input_hint: InputHint::new(vec![0]),
                })),
                prem(ast::PremKind::Let(ast::LetPrem {
                    exp_l: var("left"),
                    exp_r: var("right"),
                })),
                nested,
            ],
        },
        Span::default(),
    );
    let group = Spanned::new((id("main"), vec![rule.clone()]), Span::default());
    let else_group = Spanned::new((id("fallback"), rule.clone()), Span::default());
    let clause = Spanned::new(
        ast::ClauseKind {
            args: vec![arg(ast::ArgKind::Exp(Box::new(var("argument"))))],
            expression: var("result"),
            premises: vec![prem(ast::PremKind::Debug(ast::DebugPrem {
                exp: var("debug"),
            }))],
        },
        Span::default(),
    );
    let row = Spanned::new(
        (
            vec![arg(ast::ArgKind::Exp(Box::new(var("key"))))],
            var("value"),
        ),
        Span::default(),
    );
    let definitions = vec![
        Spanned::new(
            ast::DefKind::ExternTyp(ast::ExternTyp {
                id: id("Syntax"),
                hints: vec![],
            }),
            Span::default(),
        ),
        Spanned::new(
            ast::DefKind::Typ(ast::TypDef {
                id: id("Alias"),
                tparams: vec![Spanned::new("T".into(), Span::default())],
                def_typ: Spanned::new(
                    ast::DefTypKind::Variant(vec![(
                        not_typ(),
                        Spanned::new((id("Origin"), vec![]), Span::default()),
                        vec![],
                    )]),
                    Span::default(),
                ),
                hints: vec![],
            }),
            Span::default(),
        ),
        Spanned::new(
            ast::DefKind::Var(ast::VarDef {
                id: id("value"),
                typ: typ(),
                hints: vec![],
            }),
            Span::default(),
        ),
        Spanned::new(
            ast::DefKind::ExternRel(ast::ExternRel {
                id: id("external"),
                not_typ: not_typ(),
                input_hint: InputHint::new(vec![]),
                hints: vec![],
            }),
            Span::default(),
        ),
        Spanned::new(
            ast::DefKind::Rel(ast::Rel {
                id: id("relation"),
                not_typ: not_typ(),
                input_hint: InputHint::new(vec![]),
                rule_groups: vec![group],
                else_group: Some(else_group),
                hints: vec![],
            }),
            Span::default(),
        ),
        Spanned::new(
            ast::DefKind::ExternDec(ast::ExternDec {
                id: id("extern"),
                tparams: vec![],
                params: vec![],
                typ: typ(),
                hints: vec![],
            }),
            Span::default(),
        ),
        Spanned::new(
            ast::DefKind::BuiltinDec(ast::BuiltinDec {
                id: id("builtin"),
                tparams: vec![],
                params: vec![],
                typ: typ(),
                hints: vec![],
            }),
            Span::default(),
        ),
        Spanned::new(
            ast::DefKind::TableDec(ast::TableDec {
                id: id("table"),
                params: vec![],
                typ: typ(),
                rows: vec![row],
                hints: vec![],
            }),
            Span::default(),
        ),
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
            Span::default(),
        ),
    ];
    let rendered = Print::to_string(&definitions);
    assert_eq!(
        rendered,
        concat!(
            "extern syntax Syntax\n\n",
            "syntax Alias<T> = \n   | bool (from Origin) \n\n",
            "var value : bool\n\n",
            "extern relation external: bool\n\n",
            "relation relation: bool\n\n",
            "  rulegroup main\n\n",
            "    rule r: head\n",
            "    -- relation: input\n",
            "    -- let left = right\n",
            "    -- (if ready)*{bound <- bound*, output? -> output?*}*{bound <- bound*, output? -> output?*}\n\n",
            "  elsegroup\n\n",
            "  rulegroup fallback\n\n",
            "    rule r: head\n",
            "    -- relation: input\n",
            "    -- let left = right\n",
            "    -- (if ready)*{bound <- bound*, output? -> output?*}*{bound <- bound*, output? -> output?*}\n\n",
            "extern def $extern : bool\n\n",
            "builtin def $builtin : bool\n\n",
            "tbl def $table : bool =\n  row 0 :\n    (key) -> value\n\n",
            "def $function : bool =\n\n",
            "  clause 0 : (argument) = result\n",
            "  -- debug debug\n\n",
            "  clause -1 : (argument) = result\n",
            "  -- debug debug"
        )
    );
    assert_eq!(Print::to_string(&definitions), rendered);
    assert_eq!(Print::to_string(&[hint()][..]), " hint(meta payload)");
    assert_eq!(
        Print::to_string(&Spanned::new(
            ast::DefKind::Var(ast::VarDef {
                id: id("hidden_metadata"),
                typ: typ(),
                hints: vec![hint()],
            }),
            Span::default(),
        )),
        "var hidden_metadata : bool"
    );
}

#[test]
fn fresh_uses_aliases_collisions_wildcards_and_dimensions_deterministically() {
    let at = Span::new(Position::new("fresh", 0, 0), Position::new("fresh", 0, 0));
    let ids = names(&["bool", "bool'", "bool_1"]);
    assert_eq!(
        fresh::var_from_typ(&IdMap::new(), &ids, at.clone(), &typ())
            .id
            .node,
        "bool''"
    );
    let mut aliases = IdMap::new();
    aliases.insert(id("B"), typ());
    let alias = fresh::var_from_typ(&aliases, &IdSet::new(), at.clone(), &typ());
    assert_eq!(
        (
            alias.id.node.as_str(),
            alias.typ.node.clone(),
            alias.iters.as_slice()
        ),
        ("B", ast::TypKind::Bool, &[] as &[ast::Iter])
    );
    aliases.insert(id("C"), typ());
    assert_eq!(
        fresh::var_from_typ(&aliases, &IdSet::new(), at.clone(), &typ())
            .id
            .node,
        "bool"
    );
    assert_eq!(
        fresh::var_from_typ_wildcard(&IdMap::new(), &IdSet::new(), at.clone(), &typ())
            .id
            .node,
        "_bool"
    );
    let nested = Spanned::new(
        ast::TypKind::Iter(
            Box::new(Spanned::new(
                ast::TypKind::Iter(Box::new(typ()), ast::Iter::Opt),
                at.clone(),
            )),
            ast::Iter::List,
        ),
        at.clone(),
    );
    let nested_var = fresh::var_from_typ(&IdMap::new(), &IdSet::new(), at.clone(), &nested);
    assert_eq!(nested_var.id.node, "bool");
    assert_eq!(nested_var.typ.node, ast::TypKind::Bool);
    assert_eq!(nested_var.iters, vec![ast::Iter::Opt, ast::Iter::List]);
}

fn assert_iterated_exp(exp: &ast::Exp, dim: bool, id_span: &Span, typ_span: &Span) {
    let ast::ExpKind::Iter(inner, (ast::Iter::List, outer_binders)) = &exp.node.kind else {
        panic!("outer iteration")
    };
    assert_eq!(&exp.span, id_span);
    let ast::TypKind::Iter(outer_typ, ast::Iter::List) = &exp.node.note else {
        panic!("outer type")
    };
    assert_eq!(&outer_typ.span, id_span);
    let ast::TypKind::Iter(base_typ, ast::Iter::Opt) = &outer_typ.node else {
        panic!("inner type")
    };
    assert_eq!(base_typ.node, ast::TypKind::Bool);
    assert_eq!(&base_typ.span, id_span);
    let ast::ExpKind::Iter(base, (ast::Iter::Opt, inner_binders)) = &inner.node.kind else {
        panic!("inner iteration")
    };
    assert_eq!(&inner.span, id_span);
    assert!(
        matches!(&inner.node.note, ast::TypKind::Iter(typ, ast::Iter::Opt) if typ.node == ast::TypKind::Bool && typ.span == *id_span)
    );
    assert!(matches!(base.node.kind, ast::ExpKind::Var(_)));
    assert_eq!(base.span, *id_span);
    assert_eq!(base.node.note, ast::TypKind::Bool);
    match (dim, inner_binders.as_slice(), outer_binders.as_slice()) {
        (false, [], []) => {}
        (true, [inner], [outer]) => {
            assert_eq!(&inner.id.span, id_span);
            assert_eq!(inner.id.node, "bool");
            assert_eq!(&inner.typ.span, typ_span);
            assert!(inner.iters.is_empty());
            assert!(
                matches!(inner.typ.node, ast::TypKind::Iter(ref typ, ast::Iter::Opt) if typ.node == ast::TypKind::Bool && typ.span == *id_span)
            );
            assert_eq!(&outer.id.span, id_span);
            assert_eq!(outer.id.node, "bool");
            assert_eq!(&outer.typ.span, typ_span);
            assert_eq!(outer.iters, vec![ast::Iter::Opt]);
            assert!(
                matches!(outer.typ.node, ast::TypKind::Iter(ref typ, ast::Iter::List) if matches!(typ.node, ast::TypKind::Iter(ref base, ast::Iter::Opt) if base.node == ast::TypKind::Bool && base.span == *id_span) && typ.span == *id_span)
            );
        }
        _ => panic!("binder shape"),
    }
}

#[test]
fn fresh_exact_edges_preserve_aliases_regions_and_full_dimension_shapes() {
    let at = Span::new(
        Position::new("requested", 0, 0),
        Position::new("requested", 0, 0),
    );
    let alias_typ = Spanned::new(
        ast::TypKind::Bool,
        Span::new(
            Position::new("alias_type", 0, 0),
            Position::new("alias_type", 0, 0),
        ),
    );
    let mut aliases = IdMap::new();
    aliases.insert(id("bool"), alias_typ.clone());
    let rejected = fresh::var_from_typ(&aliases, &IdSet::new(), at.clone(), &typ());
    assert_eq!(rejected.id.node, "bool");
    assert_eq!(rejected.id.span, at);
    assert_eq!(rejected.typ.span, Span::default());
    aliases.clear();
    aliases.insert(id("Alias"), alias_typ.clone());
    let selected = fresh::var_from_typ(
        &aliases,
        &IdSet::new(),
        Span::new(
            Position::new("requested_alias", 0, 0),
            Position::new("requested_alias", 0, 0),
        ),
        &typ(),
    );
    assert_eq!(selected.id.node, "Alias");
    assert_eq!(
        selected.id.span,
        Span::new(
            Position::new("requested_alias", 0, 0),
            Position::new("requested_alias", 0, 0)
        )
    );
    assert_eq!(selected.typ, alias_typ);
    assert!(selected.iters.is_empty());
    let collision_ids = names(&["_bool", "_bool'"]);
    let wildcard = fresh::var_from_typ_wildcard(
        &IdMap::new(),
        &collision_ids,
        Span::new(
            Position::new("wildcard", 0, 0),
            Position::new("wildcard", 0, 0),
        ),
        &typ(),
    );
    assert_eq!(wildcard.id.node, "_bool''");
    assert_eq!(
        wildcard.id.span,
        Span::new(
            Position::new("wildcard", 0, 0),
            Position::new("wildcard", 0, 0)
        )
    );
    let iter_bool = Spanned::new(
        ast::TypKind::Iter(Box::new(typ()), ast::Iter::List),
        Span::new(
            Position::new("iter_type", 0, 0),
            Position::new("iter_type", 0, 0),
        ),
    );
    let inside_iter = fresh::var_from_typ(
        &aliases,
        &IdSet::new(),
        Span::new(
            Position::new("inside_iter", 0, 0),
            Position::new("inside_iter", 0, 0),
        ),
        &iter_bool,
    );
    assert_eq!(inside_iter.id.node, "Alias");
    assert_eq!(
        inside_iter.id.span,
        Span::new(
            Position::new("inside_iter", 0, 0),
            Position::new("inside_iter", 0, 0)
        )
    );
    assert_eq!(
        inside_iter.typ,
        Spanned::new(
            ast::TypKind::Bool,
            Span::new(
                Position::new("alias_type", 0, 0),
                Position::new("alias_type", 0, 0)
            )
        )
    );
    assert_eq!(inside_iter.iters, vec![ast::Iter::List]);
    let base_typ = Spanned::new(
        ast::TypKind::Bool,
        Span::new(
            Position::new("base_type", 0, 0),
            Position::new("base_type", 0, 0),
        ),
    );
    let nested = Spanned::new(
        ast::TypKind::Iter(
            Box::new(Spanned::new(
                ast::TypKind::Iter(Box::new(base_typ.clone()), ast::Iter::Opt),
                Span::new(
                    Position::new("nested_inner", 0, 0),
                    Position::new("nested_inner", 0, 0),
                ),
            )),
            ast::Iter::List,
        ),
        Span::new(
            Position::new("nested_type", 0, 0),
            Position::new("nested_type", 0, 0),
        ),
    );
    for dim in [false, true] {
        let (ids, expression) = fresh::exp_from_typ(dim, &IdMap::new(), &IdSet::new(), &nested);
        assert_eq!(ids, names(&["bool"]));
        assert_iterated_exp(&expression, dim, &nested.span, &base_typ.span);
    }
}
