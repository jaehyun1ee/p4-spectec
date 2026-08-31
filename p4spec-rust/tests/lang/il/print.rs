use super::*;

#[test]
fn test_printer_renders_nested_premises_and_definition_spec_goldens() {
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
    let rule = p4spec_rust::phrase! { node: ast::RuleKind {
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
    }, span: Span::default() };
    let group = p4spec_rust::phrase! {
        node: (id("main"), vec![rule.clone()]),
        span: Span::default(),
    };
    let else_group = p4spec_rust::phrase! {
        node: (id("fallback"), rule.clone()),
        span: Span::default(),
    };
    let clause = p4spec_rust::phrase! { node: ast::ClauseKind {
        args: vec![arg(ast::ArgKind::Exp(Box::new(var("argument"))))],
        expression: var("result"),
        premises: vec![prem(ast::PremKind::Debug(ast::DebugPrem {
            exp: var("debug"),
        }))],
    }, span: Span::default() };
    let row = p4spec_rust::phrase! { node: (
        vec![arg(ast::ArgKind::Exp(Box::new(var("key"))))],
        var("value"),
    ), span: Span::default() };
    let definitions = vec![
        p4spec_rust::phrase! { node: ast::DefKind::ExternTyp(ast::ExternTyp {
            id: id("Syntax"),
            hints: vec![],
        }), span: Span::default() },
        p4spec_rust::phrase! { node: ast::DefKind::Typ(ast::TypDef {
            id: id("Alias"),
            tparams: vec![p4spec_rust::phrase! {
                node: "T".into(),
                span: Span::default(),
            }],
            def_typ: p4spec_rust::phrase! {
                node: ast::DefTypKind::Variant(vec![(
                    not_typ(),
                    p4spec_rust::phrase! {
                        node: (id("Origin"), vec![]),
                        span: Span::default(),
                    },
                    vec![],
                )]),
                span: Span::default(),
            },
            hints: vec![],
        }), span: Span::default() },
        p4spec_rust::phrase! { node: ast::DefKind::Var(ast::VarDef {
            id: id("value"),
            typ: typ(),
            hints: vec![],
        }), span: Span::default() },
        p4spec_rust::phrase! { node: ast::DefKind::ExternRel(ast::ExternRel {
            id: id("external"),
            not_typ: not_typ(),
            input_hint: InputHint::new(vec![]),
            hints: vec![],
        }), span: Span::default() },
        p4spec_rust::phrase! { node: ast::DefKind::Rel(ast::Rel {
            id: id("relation"),
            not_typ: not_typ(),
            input_hint: InputHint::new(vec![]),
            rule_groups: vec![group],
            else_group: Some(else_group),
            hints: vec![],
        }), span: Span::default() },
        p4spec_rust::phrase! { node: ast::DefKind::ExternDec(ast::ExternDec {
            id: id("extern"),
            tparams: vec![],
            params: vec![],
            typ: typ(),
            hints: vec![],
        }), span: Span::default() },
        p4spec_rust::phrase! { node: ast::DefKind::BuiltinDec(ast::BuiltinDec {
            id: id("builtin"),
            tparams: vec![],
            params: vec![],
            typ: typ(),
            hints: vec![],
        }), span: Span::default() },
        p4spec_rust::phrase! { node: ast::DefKind::TableDec(ast::TableDec {
            id: id("table"),
            params: vec![],
            typ: typ(),
            rows: vec![row],
            hints: vec![],
        }), span: Span::default() },
        p4spec_rust::phrase! { node: ast::DefKind::FuncDec(ast::FuncDec {
            id: id("function"),
            tparams: vec![],
            params: vec![],
            typ: typ(),
            clauses: vec![clause.clone()],
            else_clause: Some(clause),
            hints: vec![],
        }), span: Span::default() },
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
        Print::to_string(
            &p4spec_rust::phrase! { node: ast::DefKind::Var(ast::VarDef {
                id: id("hidden_metadata"),
                typ: typ(),
                hints: vec![hint()],
            }), span: Span::default() }
        ),
        "var hidden_metadata : bool"
    );
}
