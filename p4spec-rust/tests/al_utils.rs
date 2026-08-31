use p4spec_rust::{
    lang::common::source::{Position, Span},
    lang::{
        al,
        common::{
            ds::{map::IdMap, set::IdSet},
            notation::{atom::Atom, mixfix::Mixfix},
        },
        el,
        hints::input::InputHint,
        il,
        traits::{eq::SyntaxEq, free::Free, print::Print},
        xl::num,
    },
};

fn span(name: &str) -> Span {
    Span::new(Position::new(name, 0, 0), Position::new(name, 0, 0))
}

fn id(name: &str) -> il::ast::Id {
    p4spec_rust::phrase! {
        node: name.to_owned(),
        span: span(name),
    }
}

fn typ() -> il::ast::Typ {
    p4spec_rust::phrase! {
        node: il::ast::TypKind::Bool,
        span: span("type"),
    }
}

fn not_typ() -> il::ast::NotTyp {
    p4spec_rust::phrase! {
        node: Mixfix::Seq(Vec::new()),
        span: span("notation"),
    }
}

fn atom() -> il::ast::Atom {
    p4spec_rust::phrase! {
        node: Atom::Keyword("A".to_owned()),
        span: span("atom"),
    }
}

fn variable(name: &str) -> il::ast::Exp {
    p4spec_rust::note_phrase! {
        node: il::ast::ExpKind::Var(id(name)),
        note: il::ast::TypKind::Bool,
        span: span(name),
    }
}

fn expr(kind: il::ast::ExpKind) -> il::ast::Exp {
    p4spec_rust::note_phrase! {
        node: kind,
        note: il::ast::TypKind::Bool,
        span: span("expression"),
    }
}

fn not_exp(name: &str) -> il::ast::NotExp {
    Mixfix::Arg(variable(name))
}

fn arg_exp(name: &str) -> il::ast::Arg {
    p4spec_rust::phrase! {
        node: il::ast::ArgKind::Exp(Box::new(variable(name))),
        span: span("arg"),
    }
}

fn path_with(name: &str) -> il::ast::Path {
    p4spec_rust::note_phrase! { node: il::ast::PathKind::Idx(
    Box::new(p4spec_rust::note_phrase! {
        node: il::ast::PathKind::Root,
        note: il::ast::TypKind::Bool,
        span: span("root"),
    }),
    Box::new(variable(name)),
    ), note: il::ast::TypKind::Bool, span: span("path") }
}

fn ids(names: &[&str]) -> IdSet {
    names.iter().map(|name| id(name)).collect()
}
#[test]
fn syntax_equality_ignores_spans_and_subcheck_strategy() {
    let exp_l = p4spec_rust::note_phrase! { node: il::ast::ExpKind::Sub(
    Box::new(variable("x")),
    typ(),
    Box::new(il::ast::Subcheck::Skip),
    ), note: il::ast::TypKind::Bool, span: span("left") };
    let exp_r = p4spec_rust::note_phrase! { node: il::ast::ExpKind::Sub(
    Box::new(variable("x")),
    typ(),
    Box::new(il::ast::Subcheck::Recurse(typ())),
    ), note: il::ast::TypKind::Text, span: span("right") };

    assert!(exp_l.syntax_eq(&exp_r));
    assert!(
        std::slice::from_ref(&il::ast::Subcheck::Skip)
            .syntax_eq(&[il::ast::Subcheck::Recurse(typ()), il::ast::Subcheck::Skip,])
    );
    assert!(id("name").syntax_eq(&id("name")));
    assert!(arg_exp("x").syntax_eq(&arg_exp("x")));
    assert!(
        p4spec_rust::phrase! {
            node: il::ast::PremKind::If(il::ast::IfPrem { exp: variable("x") }),
            span: span("prem"),
        }
        .syntax_eq(&p4spec_rust::phrase! {
            node: il::ast::PremKind::If(il::ast::IfPrem { exp: variable("x") }),
            span: span("other-prem"),
        })
    );
}

#[test]
fn syntax_equality_distinguishes_recursive_operands_variants_and_collection_rules() {
    let value = |kind| {
        p4spec_rust::note_phrase! {
            node: kind,
            note: il::ast::TypKind::Bool,
            span: span("value"),
        }
    };
    let value_recursive = value(il::ast::ValueKind::List(vec![value(
        il::ast::ValueKind::Struct(vec![(atom(), value(il::ast::ValueKind::Bool(true)))]),
    )]));
    let value_recursive_changed = value(il::ast::ValueKind::List(vec![value(
        il::ast::ValueKind::Struct(vec![(atom(), value(il::ast::ValueKind::Bool(false)))]),
    )]));
    let exp_cases = [
        (variable("x"), variable("x"), true),
        (variable("x"), variable("y"), false),
        (
            expr(il::ast::ExpKind::Tuple(vec![variable("x")])),
            expr(il::ast::ExpKind::Tuple(vec![variable("x"), variable("y")])),
            false,
        ),
    ];
    for (exp_l, exp_r, is_equal) in exp_cases {
        assert_eq!(exp_l.syntax_eq(&exp_r), is_equal);
    }
    assert!(!value_recursive.syntax_eq(&value_recursive_changed));
    assert!(
        !value(il::ast::ValueKind::Bool(true))
            .syntax_eq(&value(il::ast::ValueKind::Text("true".to_owned())))
    );

    let path_root = || {
        p4spec_rust::note_phrase! {
            node: il::ast::PathKind::Root,
            note: il::ast::TypKind::Bool,
            span: span("root"),
        }
    };
    let path_x = p4spec_rust::note_phrase! {
        node: il::ast::PathKind::Idx(Box::new(path_root()), Box::new(variable("x"))),
        note: il::ast::TypKind::Bool,
        span: span("path-x"),
    };
    let path_y = p4spec_rust::note_phrase! {
        node: il::ast::PathKind::Idx(Box::new(path_root()), Box::new(variable("y"))),
        note: il::ast::TypKind::Bool,
        span: span("path-y"),
    };
    assert!(path_x.syntax_eq(&path_x));
    assert!(!path_x.syntax_eq(&path_y));
    assert!(
        !il::ast::Pattern::List(il::ast::ListPattern::Nil)
            .syntax_eq(&il::ast::Pattern::List(il::ast::ListPattern::Cons))
    );

    let prem_rule = |input_hint| {
        p4spec_rust::phrase! { node: il::ast::PremKind::Rule(il::ast::RulePrem {
            id: id("r"),
            not_exp: not_exp("x"),
            input_hint,
        }), span: span("rule") }
    };
    assert!(prem_rule(InputHint::new(vec![0])).syntax_eq(&prem_rule(InputHint::new(vec![0]))));
    assert!(!prem_rule(InputHint::new(vec![0])).syntax_eq(&prem_rule(InputHint::new(vec![1]))));
    assert!(
        !p4spec_rust::phrase! {
            node: il::ast::PremKind::If(il::ast::IfPrem { exp: variable("x") }),
            span: span("if"),
        }
        .syntax_eq(&p4spec_rust::phrase! {
            node: il::ast::PremKind::Debug(il::ast::DebugPrem { exp: variable("x") }),
            span: span("debug"),
        })
    );
    let iter_prem = |vars_bound, vars_bind| il::ast::IterPrem {
        iter: il::ast::Iter::List,
        vars_bound,
        vars_bind,
    };
    let var_x = il::ast::Var {
        id: id("x"),
        typ: typ(),
        iters: Vec::new(),
    };
    let var_y = il::ast::Var {
        id: id("y"),
        typ: typ(),
        iters: Vec::new(),
    };
    assert!(
        iter_prem(vec![var_x.clone(), var_y.clone()], vec![var_x.clone()]).syntax_eq(&iter_prem(
            vec![var_y.clone(), var_x.clone()],
            vec![var_x.clone()]
        ))
    );
    assert!(
        !iter_prem(vec![var_x.clone()], vec![var_x.clone()])
            .syntax_eq(&iter_prem(vec![var_y.clone()], vec![var_x.clone()]))
    );
    assert!(
        !iter_prem(vec![var_x.clone()], vec![var_x.clone()])
            .syntax_eq(&iter_prem(vec![var_x.clone()], vec![var_y.clone()]))
    );
    assert!(![variable("x"), variable("y")].syntax_eq(&[variable("y"), variable("x")]));
    assert!([var_x.clone(), var_y.clone()].syntax_eq(&[var_y, var_x]));
    assert!(!std::slice::from_ref(&value_recursive).syntax_eq(&[value_recursive_changed]));
    assert!(![arg_exp("x")].syntax_eq(&[arg_exp("y")]));
}

#[test]
fn free_expression_path_argument_and_premise_variants_collect_identifier_text() {
    let x = || Box::new(variable("x"));
    let expressions = vec![
        (expr(il::ast::ExpKind::Bool(true)), ids(&[])),
        (
            expr(il::ast::ExpKind::Num(num::Number::Nat(0.into()))),
            ids(&[]),
        ),
        (expr(il::ast::ExpKind::Text("text".to_owned())), ids(&[])),
        (variable("x"), ids(&["x"])),
        (
            expr(il::ast::ExpKind::Un(
                il::ast::UnOp::Bool(p4spec_rust::lang::xl::bool::UnOp::Not),
                il::ast::OpTyp::Bool,
                x(),
            )),
            ids(&["x"]),
        ),
        (
            expr(il::ast::ExpKind::Bin(
                il::ast::BinOp::Bool(p4spec_rust::lang::xl::bool::BinOp::And),
                il::ast::OpTyp::Bool,
                x(),
                x(),
            )),
            ids(&["x"]),
        ),
        (
            expr(il::ast::ExpKind::Cmp(
                il::ast::CmpOp::Bool(p4spec_rust::lang::xl::bool::CmpOp::Eq),
                il::ast::OpTyp::Bool,
                x(),
                x(),
            )),
            ids(&["x"]),
        ),
        (expr(il::ast::ExpKind::UpCast(typ(), x())), ids(&["x"])),
        (expr(il::ast::ExpKind::DownCast(typ(), x())), ids(&["x"])),
        (
            expr(il::ast::ExpKind::Sub(
                x(),
                typ(),
                Box::new(il::ast::Subcheck::Skip),
            )),
            ids(&["x"]),
        ),
        (
            expr(il::ast::ExpKind::Match(
                x(),
                il::ast::Pattern::List(il::ast::ListPattern::Nil),
            )),
            ids(&["x"]),
        ),
        (
            expr(il::ast::ExpKind::Tuple(vec![variable("x")])),
            ids(&["x"]),
        ),
        (
            expr(il::ast::ExpKind::Case(Box::new(not_exp("x")))),
            ids(&["x"]),
        ),
        (
            expr(il::ast::ExpKind::Str(vec![(atom(), variable("x"))])),
            ids(&["x"]),
        ),
        (expr(il::ast::ExpKind::Opt(Some(x()))), ids(&["x"])),
        (expr(il::ast::ExpKind::Opt(None)), ids(&[])),
        (
            expr(il::ast::ExpKind::List(vec![variable("x")])),
            ids(&["x"]),
        ),
        (expr(il::ast::ExpKind::Cons(x(), x())), ids(&["x"])),
        (expr(il::ast::ExpKind::Cat(x(), x())), ids(&["x"])),
        (expr(il::ast::ExpKind::Mem(x(), x())), ids(&["x"])),
        (expr(il::ast::ExpKind::Len(x())), ids(&["x"])),
        (expr(il::ast::ExpKind::Dot(x(), atom())), ids(&["x"])),
        (expr(il::ast::ExpKind::Idx(x(), x())), ids(&["x"])),
        (expr(il::ast::ExpKind::Slice(x(), x(), x())), ids(&["x"])),
        (
            expr(il::ast::ExpKind::Upd(x(), path_with("x"), x())),
            ids(&["x"]),
        ),
        (
            expr(il::ast::ExpKind::Call(
                id("call"),
                Vec::new(),
                vec![arg_exp("x")],
            )),
            ids(&["x"]),
        ),
        (
            expr(il::ast::ExpKind::Iter(
                x(),
                (il::ast::Iter::List, Vec::new()),
            )),
            ids(&["x"]),
        ),
    ];
    for (expression, expected) in expressions {
        assert_eq!(expression.free(), expected);
    }

    let paths = vec![
        (
            p4spec_rust::note_phrase! {
                node: il::ast::PathKind::Root,
                note: il::ast::TypKind::Bool,
                span: span("root"),
            },
            ids(&[]),
        ),
        (path_with("x"), ids(&["x"])),
        (
            p4spec_rust::note_phrase! { node: il::ast::PathKind::Slice(
            Box::new(path_with("x")),
            Box::new(variable("y")),
            Box::new(variable("z")),
            ), note: il::ast::TypKind::Bool, span: span("slice") },
            ids(&["x", "y", "z"]),
        ),
        (
            p4spec_rust::note_phrase! {
                node: il::ast::PathKind::Dot(Box::new(path_with("x")), atom()),
                note: il::ast::TypKind::Bool,
                span: span("dot"),
            },
            ids(&["x"]),
        ),
    ];
    for (path, expected) in paths {
        assert_eq!(path.free(), expected);
    }

    assert_eq!(arg_exp("x").free(), ids(&["x"]));
    assert_eq!(
        p4spec_rust::phrase! {
            node: il::ast::ArgKind::Def(id("x")),
            span: span("def"),
        }
        .free(),
        ids(&[])
    );
    let premises = vec![
        (
            il::ast::PremKind::Rule(il::ast::RulePrem {
                id: id("r"),
                not_exp: not_exp("x"),
                input_hint: InputHint::new(Vec::new()),
            }),
            ids(&["x"]),
        ),
        (
            il::ast::PremKind::If(il::ast::IfPrem { exp: variable("x") }),
            ids(&["x"]),
        ),
        (
            il::ast::PremKind::IfHold(il::ast::IfHoldPrem {
                id: id("r"),
                not_exp: not_exp("x"),
            }),
            ids(&["x"]),
        ),
        (
            il::ast::PremKind::IfNotHold(il::ast::IfNotHoldPrem {
                id: id("r"),
                not_exp: not_exp("x"),
            }),
            ids(&["x"]),
        ),
        (
            il::ast::PremKind::Let(il::ast::LetPrem {
                exp_l: variable("x"),
                exp_r: variable("y"),
            }),
            ids(&["x", "y"]),
        ),
        (
            il::ast::PremKind::Iter(il::ast::IteratedPrem {
                prem: Box::new(p4spec_rust::phrase! {
                    node: il::ast::PremKind::If(il::ast::IfPrem { exp: variable("x") }),
                    span: span("nested"),
                }),
                iter_prem: il::ast::IterPrem {
                    iter: il::ast::Iter::List,
                    vars_bound: Vec::new(),
                    vars_bind: Vec::new(),
                },
            }),
            ids(&["x"]),
        ),
        (
            il::ast::PremKind::Debug(il::ast::DebugPrem { exp: variable("x") }),
            ids(&["x"]),
        ),
    ];
    for (premise, expected) in premises {
        assert_eq!(
            p4spec_rust::phrase! {
                node: premise,
                span: span("premise"),
            }
            .free(),
            expected
        );
    }
}

#[test]
fn free_al_shapes_and_definition_arms_are_exhaustive() {
    let premise = || {
        p4spec_rust::phrase! {
            node: il::ast::PremKind::If(il::ast::IfPrem { exp: variable("p") }),
            span: span("premise"),
        }
    };
    let rule_match = al::ast::RuleMatch {
        exps_signature: vec![variable("s")],
        exps_input: vec![variable("i")],
        prems: vec![premise()],
    };
    let rule_path = al::ast::RulePath {
        id: id("rule"),
        prems: vec![premise()],
        exps_output: vec![variable("o")],
    };
    let group: al::ast::RuleGroup = p4spec_rust::phrase! { node: al::ast::RuleGroupKind {
        id: id("group"),
        rule_match: rule_match.clone(),
        rule_paths: vec![rule_path.clone()],
    }, span: span("group") };
    let else_group: al::ast::ElseGroup = p4spec_rust::phrase! { node: al::ast::ElseGroupKind {
        id: id("else"),
        rule_match: rule_match.clone(),
        rule_path: rule_path.clone(),
    }, span: span("else") };
    let clause: al::ast::Clause = p4spec_rust::phrase! { node: il::ast::ClauseKind {
        args: vec![arg_exp("a")],
        expression: variable("c"),
        premises: vec![premise()],
    }, span: span("clause") };
    let table: al::ast::TableRow = p4spec_rust::phrase! { node: al::ast::TableRowKind {
        exps_signature: vec![variable("signature")],
        args: vec![arg_exp("a")],
        exp: variable("t"),
        prems: vec![premise()],
    }, span: span("table") };

    assert_eq!(rule_match.free(), ids(&["s", "i", "p"]));
    assert_eq!(rule_path.free(), ids(&["p", "o"]));
    assert_eq!(group.free(), ids(&["s", "i", "p", "o"]));
    assert_eq!(else_group.free(), ids(&["s", "i", "p", "o"]));
    assert_eq!(clause.free(), ids(&["a", "c", "p"]));
    assert_eq!(table.free(), ids(&["a", "t", "p"]));

    let def_type = p4spec_rust::phrase! {
        node: il::ast::DefTypKind::Plain(typ()),
        span: span("def-type"),
    };
    let definitions: Vec<(al::ast::Def, IdSet)> = vec![
        (
            p4spec_rust::phrase! { node: al::ast::DefKind::ExternTyp(al::ast::ExternTypDef {
                id: id("e"),
                hints: Vec::new(),
            }), span: span("def") },
            ids(&[]),
        ),
        (
            p4spec_rust::phrase! { node: al::ast::DefKind::Typ(al::ast::TypDef {
                id: id("t"),
                tparams: Vec::new(),
                def_typ: def_type,
                hints: Vec::new(),
            }), span: span("def") },
            ids(&[]),
        ),
        (
            p4spec_rust::phrase! { node: al::ast::DefKind::Var(al::ast::VarDef {
                id: id("v"),
                typ: typ(),
                hints: Vec::new(),
            }), span: span("def") },
            ids(&[]),
        ),
        (
            p4spec_rust::phrase! { node: al::ast::DefKind::ExternRel(al::ast::ExternRelDef {
                id: id("er"),
                not_typ: not_typ(),
                input_hint: InputHint::new(Vec::new()),
                hints: Vec::new(),
            }), span: span("def") },
            ids(&[]),
        ),
        (
            p4spec_rust::phrase! { node: al::ast::DefKind::Rel(al::ast::RelDef {
                id: id("r"),
                not_typ: not_typ(),
                input_hint: InputHint::new(Vec::new()),
                rule_groups: vec![group],
                else_group: Some(else_group),
                hints: Vec::new(),
            }), span: span("def") },
            ids(&["s", "i", "p", "o"]),
        ),
        (
            p4spec_rust::phrase! { node: al::ast::DefKind::ExternDec(al::ast::ExternDecDef {
                id: id("ed"),
                tparams: Vec::new(),
                params: Vec::new(),
                typ: typ(),
                hints: Vec::new(),
            }), span: span("def") },
            ids(&[]),
        ),
        (
            p4spec_rust::phrase! { node: al::ast::DefKind::BuiltinDec(al::ast::BuiltinDecDef {
                id: id("bd"),
                tparams: Vec::new(),
                params: Vec::new(),
                typ: typ(),
                hints: Vec::new(),
            }), span: span("def") },
            ids(&[]),
        ),
        (
            p4spec_rust::phrase! { node: al::ast::DefKind::TableDec(al::ast::TableDecDef {
                id: id("td"),
                params: Vec::new(),
                typ: typ(),
                table_rows: vec![table],
                hints: Vec::new(),
            }), span: span("def") },
            ids(&["a", "t", "p"]),
        ),
        (
            p4spec_rust::phrase! { node: al::ast::DefKind::FuncDec(al::ast::FuncDecDef {
                id: id("fd"),
                tparams: Vec::new(),
                params: Vec::new(),
                typ: typ(),
                clauses: vec![clause.clone()],
                else_clause: Some(clause),
                hints: Vec::new(),
            }), span: span("def") },
            ids(&["a", "c", "p"]),
        ),
    ];
    for (definition, expected) in definitions {
        assert_eq!(definition.free(), expected);
    }
}

fn text_typ() -> il::ast::Typ {
    p4spec_rust::phrase! {
        node: il::ast::TypKind::Text,
        span: span("text-type"),
    }
}

fn text_expression(text: &str) -> il::ast::Exp {
    p4spec_rust::note_phrase! {
        node: il::ast::ExpKind::Text(text.to_owned()),
        note: il::ast::TypKind::Text,
        span: span("text-expression"),
    }
}

fn keyword(name: &str) -> il::ast::Atom {
    p4spec_rust::phrase! {
        node: Atom::Keyword(name.to_owned()),
        span: span(name),
    }
}

fn notation(parts: Vec<Mixfix<il::ast::Typ>>) -> il::ast::NotTyp {
    p4spec_rust::phrase! {
        node: Mixfix::Seq(parts),
        span: span("notation"),
    }
}

fn premise(kind: il::ast::PremKind) -> il::ast::Prem {
    p4spec_rust::phrase! {
        node: kind,
        span: span("premise"),
    }
}

fn metadata_hint(metadata: &str) -> al::ast::Hint {
    (
        id(&format!("ignored-{metadata}")),
        p4spec_rust::phrase! {
            node: el::ast::ExpKind::Text(metadata.to_owned()),
            span: span(metadata),
        },
    )
}

fn composite_spec(metadata: &str, extern_inputs: Vec<i64>) -> al::ast::Spec {
    let hints = vec![metadata_hint(metadata)];
    let evaluate_notation = notation(vec![
        Mixfix::Atom(keyword("eval")),
        Mixfix::Arg(typ()),
        Mixfix::Atom(keyword("=>")),
        Mixfix::Arg(text_typ()),
    ]);
    let evaluate_match = al::ast::RuleMatch {
        exps_signature: vec![variable("signature")],
        exps_input: vec![text_expression("line\n\"\\")],
        prems: vec![premise(il::ast::PremKind::If(il::ast::IfPrem {
            exp: variable("ready"),
        }))],
    };
    let evaluate_path = al::ast::RulePath {
        id: id("success"),
        prems: vec![premise(il::ast::PremKind::Debug(il::ast::DebugPrem {
            exp: variable("trace"),
        }))],
        exps_output: vec![text_expression("done")],
    };
    let evaluate_group = p4spec_rust::phrase! { node: al::ast::RuleGroupKind {
        id: id("main"),
        rule_match: evaluate_match,
        rule_paths: vec![evaluate_path],
    }, span: span(metadata) };
    let fallback_match = al::ast::RuleMatch {
        exps_signature: vec![variable("fallback_signature")],
        exps_input: vec![variable("fallback_input")],
        prems: Vec::new(),
    };
    let fallback_path = al::ast::RulePath {
        id: id("fallback"),
        prems: Vec::new(),
        exps_output: vec![text_expression("fallback")],
    };
    let else_group = p4spec_rust::phrase! { node: al::ast::ElseGroupKind {
        id: id("fallback_group"),
        rule_match: fallback_match,
        rule_path: fallback_path,
    }, span: span(metadata) };
    let ready_notation = notation(vec![Mixfix::Atom(keyword("ready")), Mixfix::Arg(typ())]);
    let ready_group = p4spec_rust::phrase! { node: al::ast::RuleGroupKind {
        id: id("ready_group"),
        rule_match: al::ast::RuleMatch {
            exps_signature: vec![variable("ready_signature")],
            exps_input: vec![variable("ready_input")],
            prems: Vec::new(),
        },
        rule_paths: vec![al::ast::RulePath {
            id: id("holds"),
            prems: Vec::new(),
            exps_output: Vec::new(),
        }],
    }, span: span(metadata) };
    let table_row = p4spec_rust::phrase! { node: al::ast::TableRowKind {
        exps_signature: vec![variable("table_signature")],
        args: vec![arg_exp("key")],
        exp: text_expression("row\tvalue"),
        prems: vec![premise(il::ast::PremKind::If(il::ast::IfPrem {
            exp: variable("ready"),
        }))],
    }, span: span(metadata) };
    let function_clause = p4spec_rust::phrase! { node: il::ast::ClauseKind {
        args: vec![arg_exp("argument")],
        expression: text_expression("quoted\"\\"),
        premises: vec![premise(il::ast::PremKind::If(il::ast::IfPrem {
            exp: variable("ready"),
        }))],
    }, span: span(metadata) };
    let else_clause = p4spec_rust::phrase! { node: il::ast::ClauseKind {
        args: vec![arg_exp("fallback")],
        expression: expr(il::ast::ExpKind::Bool(false)),
        premises: Vec::new(),
    }, span: span(metadata) };
    let def_type = p4spec_rust::phrase! {
        node: il::ast::DefTypKind::Plain(typ()),
        span: span("defined-type"),
    };
    let parameter = p4spec_rust::phrase! {
        node: il::ast::ParamKind::Exp(typ()),
        span: span("parameter"),
    };

    vec![
        p4spec_rust::phrase! { node: al::ast::DefKind::ExternTyp(al::ast::ExternTypDef {
            id: id("External"),
            hints: hints.clone(),
        }), span: span(metadata) },
        p4spec_rust::phrase! { node: al::ast::DefKind::Typ(al::ast::TypDef {
            id: id("Box"),
            tparams: vec![id("T")],
            def_typ: def_type,
            hints: hints.clone(),
        }), span: span(metadata) },
        p4spec_rust::phrase! { node: al::ast::DefKind::Var(al::ast::VarDef {
            id: id("state"),
            typ: typ(),
            hints: hints.clone(),
        }), span: span(metadata) },
        p4spec_rust::phrase! { node: al::ast::DefKind::ExternRel(al::ast::ExternRelDef {
            id: id("Check"),
            not_typ: notation(vec![Mixfix::Atom(keyword("check")), Mixfix::Arg(typ())]),
            input_hint: InputHint::new(extern_inputs),
            hints: hints.clone(),
        }), span: span(metadata) },
        p4spec_rust::phrase! { node: al::ast::DefKind::Rel(al::ast::RelDef {
            id: id("Evaluate"),
            not_typ: evaluate_notation,
            input_hint: InputHint::new(vec![0]),
            rule_groups: vec![evaluate_group],
            else_group: Some(else_group),
            hints: hints.clone(),
        }), span: span(metadata) },
        p4spec_rust::phrase! { node: al::ast::DefKind::Rel(al::ast::RelDef {
            id: id("Ready"),
            not_typ: ready_notation,
            input_hint: InputHint::new(vec![0]),
            rule_groups: vec![ready_group],
            else_group: None,
            hints: hints.clone(),
        }), span: span(metadata) },
        p4spec_rust::phrase! { node: al::ast::DefKind::ExternDec(al::ast::ExternDecDef {
            id: id("external"),
            tparams: Vec::new(),
            params: vec![parameter.clone()],
            typ: typ(),
            hints: hints.clone(),
        }), span: span(metadata) },
        p4spec_rust::phrase! { node: al::ast::DefKind::BuiltinDec(al::ast::BuiltinDecDef {
            id: id("builtin"),
            tparams: Vec::new(),
            params: vec![parameter.clone()],
            typ: typ(),
            hints: hints.clone(),
        }), span: span(metadata) },
        p4spec_rust::phrase! { node: al::ast::DefKind::TableDec(al::ast::TableDecDef {
            id: id("lookup"),
            params: vec![parameter.clone()],
            typ: typ(),
            table_rows: vec![table_row],
            hints: hints.clone(),
        }), span: span(metadata) },
        p4spec_rust::phrase! { node: al::ast::DefKind::FuncDec(al::ast::FuncDecDef {
            id: id("run"),
            tparams: vec![id("T")],
            params: vec![parameter],
            typ: typ(),
            clauses: vec![function_clause],
            else_clause: Some(else_clause),
            hints,
        }), span: span(metadata) },
    ]
}

#[test]
fn composite_al_spec_prints_in_ocaml_order_with_exact_spacing_and_escaping() {
    let spec = composite_spec("source-a", vec![0]);

    assert_eq!(
        Print::to_string(&spec),
        concat!(
            "extern syntax External\n\n",
            "syntax Box<T> = bool\n\n",
            "var state : bool\n\n",
            "extern relation Check: check bool\n\n",
            "relation Evaluate: eval bool => text\n\n",
            "  rulegroup main\n\n",
            "   match\n\n",
            "    (signature) eval signature => %\n",
            "    eval \"line\\n\\\"\\\\\" => %\n",
            "    -- if ready\n\n",
            "   paths\n\n",
            "    rulepath success\n",
            "    -- debug trace\n",
            "    -- output: eval % => \"done\"\n\n",
            "  elsegroup\n\n",
            "  rulegroup fallback_group\n\n",
            "   match\n\n",
            "    (signature) eval fallback_signature => %\n",
            "    eval fallback_input => %\n\n",
            "   paths\n\n",
            "    rulepath fallback\n",
            "    -- output: eval % => \"fallback\"\n\n",
            "relation Ready: ready bool\n\n",
            "  rulegroup ready_group\n\n",
            "   match\n\n",
            "    (signature) ready ready_signature\n",
            "    ready ready_input\n\n",
            "   paths\n\n",
            "    rulepath holds\n",
            "    -- the relation holds\n\n",
            "extern def $external(bool) : bool\n\n",
            "builtin def $builtin(bool) : bool\n\n",
            "tbl def $lookup(bool) : bool =\n",
            "  row 0 :\n",
            "    (signature) table_signature\n",
            "    (key) -> \"row\\tvalue\"\n",
            "    -- if ready\n\n",
            "def $run<T>(bool) : bool =\n\n",
            "  clause 0 : (argument) = \"quoted\\\"\\\\\"\n",
            "  -- if ready\n\n",
            "  clause -1 : (fallback) = false",
        )
    );
}

#[test]
fn composite_al_spec_omits_source_hints_and_extern_relation_inputs() {
    let first = composite_spec("source-a", vec![0]);
    let changed_metadata = composite_spec("source-b", vec![7, 9]);

    assert_eq!(
        Print::to_string(&first),
        Print::to_string(&changed_metadata)
    );
}

#[test]
fn fresh_names_combine_aliases_collisions_wildcards_and_nested_dimensions() {
    let requested = span("requested");
    let alias_typ = p4spec_rust::phrase! {
        node: il::ast::TypKind::Bool,
        span: span("alias-type"),
    };
    let nested = p4spec_rust::phrase! { node: il::ast::TypKind::Iter(
        Box::new(p4spec_rust::phrase! {
            node:
            il::ast::TypKind::Iter(Box::new(typ()), il::ast::Iter::Opt),
            span: span("inner-iteration"),
        }),
        il::ast::Iter::List,
    ), span: span("outer-iteration") };
    let mut aliases = IdMap::new();
    aliases.insert(id("Alias"), alias_typ.clone());

    let variable = il::fresh::var_from_typ(
        &aliases,
        &ids(&["Alias", "Alias'", "Alias_1"]),
        requested.clone(),
        &nested,
    );
    assert_eq!(variable.id.node, "Alias''");
    assert_eq!(variable.id.span, requested);
    assert_eq!(variable.typ, alias_typ);
    assert_eq!(
        variable.iters,
        vec![il::ast::Iter::Opt, il::ast::Iter::List]
    );

    aliases.insert(
        id("Other"),
        p4spec_rust::phrase! {
            node: il::ast::TypKind::Bool,
            span: span("other-type"),
        },
    );
    let wildcard = il::fresh::var_from_typ_wildcard(
        &aliases,
        &ids(&["_bool", "_bool'", "_bool_1"]),
        requested.clone(),
        &nested,
    );
    assert_eq!(wildcard.id.node, "_bool''");
    assert_eq!(wildcard.id.span, requested);
    assert_eq!(wildcard.typ.node, il::ast::TypKind::Bool);
    assert_eq!(
        wildcard.iters,
        vec![il::ast::Iter::Opt, il::ast::Iter::List]
    );

    let (generated_ids, generated) =
        il::fresh::exp_from_typ(true, &aliases, &ids(&["bool"]), &nested);
    assert_eq!(generated_ids, ids(&["bool", "bool'"]));
    let il::ast::ExpKind::Iter(inner, (il::ast::Iter::List, outer_binders)) = generated.node else {
        panic!("outer iteration")
    };
    let il::ast::ExpKind::Iter(_, (il::ast::Iter::Opt, inner_binders)) = inner.node else {
        panic!("inner iteration")
    };
    assert_eq!(inner_binders.len(), 1);
    assert_eq!(outer_binders.len(), 1);
    assert!(inner_binders[0].iters.is_empty());
    assert_eq!(outer_binders[0].iters, vec![il::ast::Iter::Opt]);
}
