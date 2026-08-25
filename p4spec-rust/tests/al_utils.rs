use std::collections::{BTreeMap, BTreeSet};

use p4spec_rust::{
    domain::{
        atom::Atom,
        mixfix::Mixfix,
        source::{Position, Span, Spanned},
    },
    lang::{al, el, hints::input::InputHint, il, xl::num},
};

fn span(name: &str) -> Span {
    Span::new(Position::new(name, 0, 0), Position::new(name, 0, 0))
}

fn id(name: &str) -> il::ast::Id {
    Spanned::new(name.to_owned(), span(name))
}

fn typ() -> il::ast::Typ {
    Spanned::new(il::ast::TypKind::BoolT, span("type"))
}

fn not_typ() -> il::ast::NotTyp {
    Spanned::new(Mixfix::Seq(Vec::new()), span("notation"))
}

fn atom() -> il::ast::Atom {
    Spanned::new(Atom::Keyword("A".to_owned()), span("atom"))
}

fn variable(name: &str) -> il::ast::Exp {
    il::ast::Exp::new(
        il::ast::ExpKind::VarE(id(name)),
        il::ast::TypKind::BoolT,
        span(name),
    )
}

fn expr(kind: il::ast::ExpKind) -> il::ast::Exp {
    il::ast::Exp::new(kind, il::ast::TypKind::BoolT, span("expression"))
}

fn not_exp(name: &str) -> il::ast::NotExp {
    Mixfix::Arg(variable(name))
}

fn arg_exp(name: &str) -> il::ast::Arg {
    Spanned::new(
        il::ast::ArgKind::ExpA(Box::new(variable(name))),
        span("arg"),
    )
}

fn path_with(name: &str) -> il::ast::Path {
    il::ast::Path::new(
        il::ast::PathKind::IdxP(
            Box::new(il::ast::Path::new(
                il::ast::PathKind::RootP,
                il::ast::TypKind::BoolT,
                span("root"),
            )),
            Box::new(variable(name)),
        ),
        il::ast::TypKind::BoolT,
        span("path"),
    )
}

fn ids(names: &[&str]) -> al::free::FreeVars {
    names.iter().map(|name| (*name).to_owned()).collect()
}
#[test]
fn al_equality_delegates_to_span_and_subcheck_insensitive_il_semantics() {
    let left = il::ast::Exp::new(
        il::ast::ExpKind::SubE(
            Box::new(variable("x")),
            typ(),
            Box::new(il::ast::Subcheck::SkipSC),
        ),
        il::ast::TypKind::BoolT,
        span("left"),
    );
    let right = il::ast::Exp::new(
        il::ast::ExpKind::SubE(
            Box::new(variable("x")),
            typ(),
            Box::new(il::ast::Subcheck::RecurseSC(typ())),
        ),
        il::ast::TypKind::TextT,
        span("right"),
    );

    assert!(al::eq::eq_exp(&left, &right));
    assert!(al::eq::eq_id(&id("name"), &id("name")));
    assert!(al::eq::eq_arg(&arg_exp("x"), &arg_exp("x")));
    assert!(al::eq::eq_prem(
        &Spanned::new(il::ast::PremKind::IfPr(variable("x")), span("prem")),
        &Spanned::new(il::ast::PremKind::IfPr(variable("x")), span("other-prem")),
    ));
}

#[test]
fn al_equality_distinguishes_recursive_operands_variants_and_collection_rules() {
    let value = |kind| il::ast::Value::new(kind, il::ast::TypKind::BoolT, span("value"));
    let recursive = value(il::ast::ValueKind::ListV(vec![value(
        il::ast::ValueKind::StructV(vec![(atom(), value(il::ast::ValueKind::BoolV(true)))]),
    )]));
    let recursive_changed = value(il::ast::ValueKind::ListV(vec![value(
        il::ast::ValueKind::StructV(vec![(atom(), value(il::ast::ValueKind::BoolV(false)))]),
    )]));
    let expressions = [
        (variable("x"), variable("x"), true),
        (variable("x"), variable("y"), false),
        (
            expr(il::ast::ExpKind::TupleE(vec![variable("x")])),
            expr(il::ast::ExpKind::TupleE(vec![variable("x"), variable("y")])),
            false,
        ),
    ];
    for (left, right, expected) in expressions {
        assert_eq!(al::eq::eq_exp(&left, &right), expected);
    }
    assert!(!al::eq::eq_value(&recursive, &recursive_changed));
    assert!(!al::eq::eq_value(
        &value(il::ast::ValueKind::BoolV(true)),
        &value(il::ast::ValueKind::TextV("true".to_owned())),
    ));

    let root = || {
        il::ast::Path::new(
            il::ast::PathKind::RootP,
            il::ast::TypKind::BoolT,
            span("root"),
        )
    };
    let path_x = il::ast::Path::new(
        il::ast::PathKind::IdxP(Box::new(root()), Box::new(variable("x"))),
        il::ast::TypKind::BoolT,
        span("path-x"),
    );
    let path_y = il::ast::Path::new(
        il::ast::PathKind::IdxP(Box::new(root()), Box::new(variable("y"))),
        il::ast::TypKind::BoolT,
        span("path-y"),
    );
    assert!(al::eq::eq_path(&path_x, &path_x));
    assert!(!al::eq::eq_path(&path_x, &path_y));
    assert!(!al::eq::eq_pattern(
        &il::ast::Pattern::ListP(il::ast::ListPattern::Nil),
        &il::ast::Pattern::ListP(il::ast::ListPattern::Cons),
    ));

    let rule = |input| {
        Spanned::new(
            il::ast::PremKind::RulePr(id("r"), not_exp("x"), input),
            span("rule"),
        )
    };
    assert!(al::eq::eq_prem(
        &rule(InputHint::new(vec![0])),
        &rule(InputHint::new(vec![0]))
    ));
    assert!(!al::eq::eq_prem(
        &rule(InputHint::new(vec![0])),
        &rule(InputHint::new(vec![1]))
    ));
    assert!(!al::eq::eq_prem(
        &Spanned::new(il::ast::PremKind::IfPr(variable("x")), span("if")),
        &Spanned::new(il::ast::PremKind::DebugPr(variable("x")), span("debug")),
    ));
    let iterprem = |bound, bind| il::ast::IterPrem {
        iter: il::ast::Iter::List,
        vars_bound: bound,
        vars_bind: bind,
    };
    let x_var = il::ast::Var {
        id: id("x"),
        typ: typ(),
        iters: Vec::new(),
    };
    let y_var = il::ast::Var {
        id: id("y"),
        typ: typ(),
        iters: Vec::new(),
    };
    assert!(al::eq::eq_iterprem(
        &iterprem(vec![x_var.clone(), y_var.clone()], vec![x_var.clone()]),
        &iterprem(vec![y_var.clone(), x_var.clone()], vec![x_var.clone()]),
    ));
    assert!(!al::eq::eq_iterprem(
        &iterprem(vec![x_var.clone()], vec![x_var.clone()]),
        &iterprem(vec![y_var.clone()], vec![x_var.clone()]),
    ));
    assert!(!al::eq::eq_iterprem(
        &iterprem(vec![x_var.clone()], vec![x_var.clone()]),
        &iterprem(vec![x_var.clone()], vec![y_var.clone()]),
    ));
    assert!(!al::eq::eq_exps(
        &[variable("x"), variable("y")],
        &[variable("y"), variable("x")]
    ));
    assert!(al::eq::eq_vars(
        &[x_var.clone(), y_var.clone()],
        &[y_var, x_var],
    ));
    assert!(!al::eq::eq_values(
        std::slice::from_ref(&recursive),
        &[recursive_changed],
    ));
    assert!(!al::eq::eq_args(&[arg_exp("x")], &[arg_exp("y")]));
}

#[test]
fn free_expression_path_argument_and_premise_variants_collect_identifier_text() {
    let x = || Box::new(variable("x"));
    let expressions = vec![
        (expr(il::ast::ExpKind::BoolE(true)), ids(&[])),
        (
            expr(il::ast::ExpKind::NumE(num::Number::Nat(0.into()))),
            ids(&[]),
        ),
        (expr(il::ast::ExpKind::TextE("text".to_owned())), ids(&[])),
        (variable("x"), ids(&["x"])),
        (
            expr(il::ast::ExpKind::UnE(
                il::ast::UnOp::NotOp,
                il::ast::OpTyp::BoolT,
                x(),
            )),
            ids(&["x"]),
        ),
        (
            expr(il::ast::ExpKind::BinE(
                il::ast::BinOp::AndOp,
                il::ast::OpTyp::BoolT,
                x(),
                x(),
            )),
            ids(&["x"]),
        ),
        (
            expr(il::ast::ExpKind::CmpE(
                il::ast::CmpOp::EqOp,
                il::ast::OpTyp::BoolT,
                x(),
                x(),
            )),
            ids(&["x"]),
        ),
        (expr(il::ast::ExpKind::UpCastE(typ(), x())), ids(&["x"])),
        (expr(il::ast::ExpKind::DownCastE(typ(), x())), ids(&["x"])),
        (
            expr(il::ast::ExpKind::SubE(
                x(),
                typ(),
                Box::new(il::ast::Subcheck::SkipSC),
            )),
            ids(&["x"]),
        ),
        (
            expr(il::ast::ExpKind::MatchE(
                x(),
                il::ast::Pattern::ListP(il::ast::ListPattern::Nil),
            )),
            ids(&["x"]),
        ),
        (
            expr(il::ast::ExpKind::TupleE(vec![variable("x")])),
            ids(&["x"]),
        ),
        (
            expr(il::ast::ExpKind::CaseE(Box::new(not_exp("x")))),
            ids(&["x"]),
        ),
        (
            expr(il::ast::ExpKind::StrE(vec![(atom(), variable("x"))])),
            ids(&["x"]),
        ),
        (expr(il::ast::ExpKind::OptE(Some(x()))), ids(&["x"])),
        (expr(il::ast::ExpKind::OptE(None)), ids(&[])),
        (
            expr(il::ast::ExpKind::ListE(vec![variable("x")])),
            ids(&["x"]),
        ),
        (expr(il::ast::ExpKind::ConsE(x(), x())), ids(&["x"])),
        (expr(il::ast::ExpKind::CatE(x(), x())), ids(&["x"])),
        (expr(il::ast::ExpKind::MemE(x(), x())), ids(&["x"])),
        (expr(il::ast::ExpKind::LenE(x())), ids(&["x"])),
        (expr(il::ast::ExpKind::DotE(x(), atom())), ids(&["x"])),
        (expr(il::ast::ExpKind::IdxE(x(), x())), ids(&["x"])),
        (expr(il::ast::ExpKind::SliceE(x(), x(), x())), ids(&["x"])),
        (
            expr(il::ast::ExpKind::UpdE(x(), path_with("x"), x())),
            ids(&["x"]),
        ),
        (
            expr(il::ast::ExpKind::CallE(
                id("call"),
                Vec::new(),
                vec![arg_exp("x")],
            )),
            ids(&["x"]),
        ),
        (
            expr(il::ast::ExpKind::IterE(
                x(),
                (il::ast::Iter::List, Vec::new()),
            )),
            ids(&["x"]),
        ),
    ];
    for (expression, expected) in expressions {
        assert_eq!(al::free::free_exp(&expression), expected);
    }

    let paths = vec![
        (
            il::ast::Path::new(
                il::ast::PathKind::RootP,
                il::ast::TypKind::BoolT,
                span("root"),
            ),
            ids(&[]),
        ),
        (path_with("x"), ids(&["x"])),
        (
            il::ast::Path::new(
                il::ast::PathKind::SliceP(
                    Box::new(path_with("x")),
                    Box::new(variable("y")),
                    Box::new(variable("z")),
                ),
                il::ast::TypKind::BoolT,
                span("slice"),
            ),
            ids(&["x", "y", "z"]),
        ),
        (
            il::ast::Path::new(
                il::ast::PathKind::DotP(Box::new(path_with("x")), atom()),
                il::ast::TypKind::BoolT,
                span("dot"),
            ),
            ids(&["x"]),
        ),
    ];
    for (path, expected) in paths {
        assert_eq!(al::free::free_path(&path), expected);
    }

    assert_eq!(al::free::free_arg(&arg_exp("x")), ids(&["x"]));
    assert_eq!(
        al::free::free_arg(&Spanned::new(il::ast::ArgKind::DefA(id("x")), span("def"))),
        ids(&[])
    );
    let premises = vec![
        (
            il::ast::PremKind::RulePr(id("r"), not_exp("x"), InputHint::new(Vec::new())),
            ids(&["x"]),
        ),
        (il::ast::PremKind::IfPr(variable("x")), ids(&["x"])),
        (
            il::ast::PremKind::IfHoldPr(id("r"), not_exp("x")),
            ids(&["x"]),
        ),
        (
            il::ast::PremKind::IfNotHoldPr(id("r"), not_exp("x")),
            ids(&["x"]),
        ),
        (
            il::ast::PremKind::LetPr(variable("x"), variable("y")),
            ids(&["x", "y"]),
        ),
        (
            il::ast::PremKind::IterPr(
                Box::new(Spanned::new(
                    il::ast::PremKind::IfPr(variable("x")),
                    span("nested"),
                )),
                il::ast::IterPrem {
                    iter: il::ast::Iter::List,
                    vars_bound: Vec::new(),
                    vars_bind: Vec::new(),
                },
            ),
            ids(&["x"]),
        ),
        (il::ast::PremKind::DebugPr(variable("x")), ids(&["x"])),
    ];
    for (premise, expected) in premises {
        assert_eq!(
            al::free::free_prem(&Spanned::new(premise, span("premise"))),
            expected
        );
    }
}

#[test]
fn free_al_shapes_and_definition_arms_are_exhaustive() {
    let premise = || Spanned::new(il::ast::PremKind::IfPr(variable("p")), span("premise"));
    let rule_match = al::ast::RuleMatch {
        signature: vec![variable("s")],
        inputs: vec![variable("i")],
        premises: vec![premise()],
    };
    let rule_path = al::ast::RulePath {
        rule_id: id("rule"),
        premises: vec![premise()],
        outputs: vec![variable("o")],
    };
    let group: al::ast::RuleGroup = Spanned::new(
        al::ast::RuleGroupKind {
            id: id("group"),
            rule_match: rule_match.clone(),
            paths: vec![rule_path.clone()],
        },
        span("group"),
    );
    let else_group: al::ast::ElseGroup = Spanned::new(
        al::ast::ElseGroupKind {
            id: id("else"),
            rule_match: rule_match.clone(),
            path: rule_path.clone(),
        },
        span("else"),
    );
    let clause: al::ast::Clause = Spanned::new(
        il::ast::ClauseKind {
            args: vec![arg_exp("a")],
            expression: variable("c"),
            premises: vec![premise()],
        },
        span("clause"),
    );
    let table: al::ast::TableRow = Spanned::new(
        al::ast::TableRowKind {
            signature: vec![variable("signature")],
            args: vec![arg_exp("a")],
            expression: variable("t"),
            premises: vec![premise()],
        },
        span("table"),
    );

    assert_eq!(al::free::free_rulematch(&rule_match), ids(&["s", "i", "p"]));
    assert_eq!(al::free::free_rulepath(&rule_path), ids(&["p", "o"]));
    assert_eq!(al::free::free_rulegroup(&group), ids(&["s", "i", "p", "o"]));
    assert_eq!(
        al::free::free_elsegroup(&else_group),
        ids(&["s", "i", "p", "o"])
    );
    assert_eq!(al::free::free_clause(&clause), ids(&["a", "c", "p"]));
    assert_eq!(al::free::free_tablerow(&table), ids(&["a", "t", "p"]));

    let def_type = Spanned::new(il::ast::DefTypKind::PlainT(typ()), span("def-type"));
    let definitions: Vec<(al::ast::Def, BTreeSet<String>)> = vec![
        (
            Spanned::new(
                al::ast::DefKind::ExternTypD(id("e"), Vec::new()),
                span("def"),
            ),
            ids(&[]),
        ),
        (
            Spanned::new(
                al::ast::DefKind::TypD(id("t"), Vec::new(), def_type, Vec::new()),
                span("def"),
            ),
            ids(&[]),
        ),
        (
            Spanned::new(
                al::ast::DefKind::VarD(id("v"), typ(), Vec::new()),
                span("def"),
            ),
            ids(&[]),
        ),
        (
            Spanned::new(
                al::ast::DefKind::ExternRelD(
                    id("er"),
                    not_typ(),
                    InputHint::new(Vec::new()),
                    Vec::new(),
                ),
                span("def"),
            ),
            ids(&[]),
        ),
        (
            Spanned::new(
                al::ast::DefKind::RelD(
                    id("r"),
                    not_typ(),
                    InputHint::new(Vec::new()),
                    vec![group],
                    Some(else_group),
                    Vec::new(),
                ),
                span("def"),
            ),
            ids(&["s", "i", "p", "o"]),
        ),
        (
            Spanned::new(
                al::ast::DefKind::ExternDecD(id("ed"), Vec::new(), Vec::new(), typ(), Vec::new()),
                span("def"),
            ),
            ids(&[]),
        ),
        (
            Spanned::new(
                al::ast::DefKind::BuiltinDecD(id("bd"), Vec::new(), Vec::new(), typ(), Vec::new()),
                span("def"),
            ),
            ids(&[]),
        ),
        (
            Spanned::new(
                al::ast::DefKind::TableDecD(id("td"), Vec::new(), typ(), vec![table], Vec::new()),
                span("def"),
            ),
            ids(&["a", "t", "p"]),
        ),
        (
            Spanned::new(
                al::ast::DefKind::FuncDecD(
                    id("fd"),
                    Vec::new(),
                    Vec::new(),
                    typ(),
                    vec![clause.clone()],
                    Some(clause),
                    Vec::new(),
                ),
                span("def"),
            ),
            ids(&["a", "c", "p"]),
        ),
    ];
    for (definition, expected) in definitions {
        assert_eq!(al::free::free_def(&definition), expected);
    }
}

fn text_typ() -> il::ast::Typ {
    Spanned::new(il::ast::TypKind::TextT, span("text-type"))
}

fn text_expression(text: &str) -> il::ast::Exp {
    il::ast::Exp::new(
        il::ast::ExpKind::TextE(text.to_owned()),
        il::ast::TypKind::TextT,
        span("text-expression"),
    )
}

fn keyword(name: &str) -> il::ast::Atom {
    Spanned::new(Atom::Keyword(name.to_owned()), span(name))
}

fn notation(parts: Vec<Mixfix<il::ast::Typ>>) -> il::ast::NotTyp {
    Spanned::new(Mixfix::Seq(parts), span("notation"))
}

fn premise(kind: il::ast::PremKind) -> il::ast::Prem {
    Spanned::new(kind, span("premise"))
}

fn metadata_hint(metadata: &str) -> al::ast::Hint {
    al::ast::Hint {
        hintid: id(&format!("ignored-{metadata}")),
        hintexp: Spanned::new(el::ast::ExpKind::TextE(metadata.to_owned()), span(metadata)),
    }
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
        signature: vec![variable("signature")],
        inputs: vec![text_expression("line\n\"\\")],
        premises: vec![premise(il::ast::PremKind::IfPr(variable("ready")))],
    };
    let evaluate_path = al::ast::RulePath {
        rule_id: id("success"),
        premises: vec![premise(il::ast::PremKind::DebugPr(variable("trace")))],
        outputs: vec![text_expression("done")],
    };
    let evaluate_group = Spanned::new(
        al::ast::RuleGroupKind {
            id: id("main"),
            rule_match: evaluate_match,
            paths: vec![evaluate_path],
        },
        span(metadata),
    );
    let fallback_match = al::ast::RuleMatch {
        signature: vec![variable("fallback_signature")],
        inputs: vec![variable("fallback_input")],
        premises: Vec::new(),
    };
    let fallback_path = al::ast::RulePath {
        rule_id: id("fallback"),
        premises: Vec::new(),
        outputs: vec![text_expression("fallback")],
    };
    let else_group = Spanned::new(
        al::ast::ElseGroupKind {
            id: id("fallback_group"),
            rule_match: fallback_match,
            path: fallback_path,
        },
        span(metadata),
    );
    let ready_notation = notation(vec![Mixfix::Atom(keyword("ready")), Mixfix::Arg(typ())]);
    let ready_group = Spanned::new(
        al::ast::RuleGroupKind {
            id: id("ready_group"),
            rule_match: al::ast::RuleMatch {
                signature: vec![variable("ready_signature")],
                inputs: vec![variable("ready_input")],
                premises: Vec::new(),
            },
            paths: vec![al::ast::RulePath {
                rule_id: id("holds"),
                premises: Vec::new(),
                outputs: Vec::new(),
            }],
        },
        span(metadata),
    );
    let table_row = Spanned::new(
        al::ast::TableRowKind {
            signature: vec![variable("table_signature")],
            args: vec![arg_exp("key")],
            expression: text_expression("row\tvalue"),
            premises: vec![premise(il::ast::PremKind::IfPr(variable("ready")))],
        },
        span(metadata),
    );
    let function_clause = Spanned::new(
        il::ast::ClauseKind {
            args: vec![arg_exp("argument")],
            expression: text_expression("quoted\"\\"),
            premises: vec![premise(il::ast::PremKind::IfPr(variable("ready")))],
        },
        span(metadata),
    );
    let else_clause = Spanned::new(
        il::ast::ClauseKind {
            args: vec![arg_exp("fallback")],
            expression: expr(il::ast::ExpKind::BoolE(false)),
            premises: Vec::new(),
        },
        span(metadata),
    );
    let def_type = Spanned::new(il::ast::DefTypKind::PlainT(typ()), span("defined-type"));
    let parameter = Spanned::new(il::ast::ParamKind::ExpP(typ()), span("parameter"));

    vec![
        Spanned::new(
            al::ast::DefKind::ExternTypD(id("External"), hints.clone()),
            span(metadata),
        ),
        Spanned::new(
            al::ast::DefKind::TypD(id("Box"), vec![id("T")], def_type, hints.clone()),
            span(metadata),
        ),
        Spanned::new(
            al::ast::DefKind::VarD(id("state"), typ(), hints.clone()),
            span(metadata),
        ),
        Spanned::new(
            al::ast::DefKind::ExternRelD(
                id("Check"),
                notation(vec![Mixfix::Atom(keyword("check")), Mixfix::Arg(typ())]),
                InputHint::new(extern_inputs),
                hints.clone(),
            ),
            span(metadata),
        ),
        Spanned::new(
            al::ast::DefKind::RelD(
                id("Evaluate"),
                evaluate_notation,
                InputHint::new(vec![0]),
                vec![evaluate_group],
                Some(else_group),
                hints.clone(),
            ),
            span(metadata),
        ),
        Spanned::new(
            al::ast::DefKind::RelD(
                id("Ready"),
                ready_notation,
                InputHint::new(vec![0]),
                vec![ready_group],
                None,
                hints.clone(),
            ),
            span(metadata),
        ),
        Spanned::new(
            al::ast::DefKind::ExternDecD(
                id("external"),
                Vec::new(),
                vec![parameter.clone()],
                typ(),
                hints.clone(),
            ),
            span(metadata),
        ),
        Spanned::new(
            al::ast::DefKind::BuiltinDecD(
                id("builtin"),
                Vec::new(),
                vec![parameter.clone()],
                typ(),
                hints.clone(),
            ),
            span(metadata),
        ),
        Spanned::new(
            al::ast::DefKind::TableDecD(
                id("lookup"),
                vec![parameter.clone()],
                typ(),
                vec![table_row],
                hints.clone(),
            ),
            span(metadata),
        ),
        Spanned::new(
            al::ast::DefKind::FuncDecD(
                id("run"),
                vec![id("T")],
                vec![parameter],
                typ(),
                vec![function_clause],
                Some(else_clause),
                hints,
            ),
            span(metadata),
        ),
    ]
}

#[test]
fn composite_al_spec_prints_in_ocaml_order_with_exact_spacing_and_escaping() {
    let spec = composite_spec("source-a", vec![0]);

    assert_eq!(
        al::print::string_of_spec(&spec),
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
        al::print::string_of_spec(&first),
        al::print::string_of_spec(&changed_metadata)
    );
}

#[test]
fn fresh_names_combine_alias_regions_collisions_wildcards_and_nested_dimensions() {
    let requested = span("requested");
    let alias_region = span("alias");
    let alias_typ = Spanned::new(il::ast::TypKind::BoolT, span("alias-type"));
    let nested = Spanned::new(
        il::ast::TypKind::IterT(
            Box::new(Spanned::new(
                il::ast::TypKind::IterT(Box::new(typ()), il::ast::Iter::Opt),
                span("inner-iteration"),
            )),
            il::ast::Iter::List,
        ),
        span("outer-iteration"),
    );
    let mut aliases = BTreeMap::new();
    aliases.insert(
        "Alias".to_owned(),
        (alias_region.clone(), alias_typ.clone()),
    );

    let variable = al::fresh::var_from_typ(
        &aliases,
        &ids(&["Alias", "Alias'", "Alias_1"]),
        requested.clone(),
        &nested,
    );
    assert_eq!(variable.id.node, "Alias''");
    assert_eq!(variable.id.span, alias_region);
    assert_eq!(variable.typ, alias_typ);
    assert_eq!(
        variable.iters,
        vec![il::ast::Iter::Opt, il::ast::Iter::List]
    );

    aliases.insert(
        "Other".to_owned(),
        (
            span("other-alias"),
            Spanned::new(il::ast::TypKind::BoolT, span("other-type")),
        ),
    );
    let wildcard = al::fresh::var_from_typ_wildcard(
        &aliases,
        &ids(&["_bool", "_bool'", "_bool_1"]),
        requested.clone(),
        &nested,
    );
    assert_eq!(wildcard.id.node, "_bool''");
    assert_eq!(wildcard.id.span, requested);
    assert_eq!(wildcard.typ.node, il::ast::TypKind::BoolT);
    assert_eq!(
        wildcard.iters,
        vec![il::ast::Iter::Opt, il::ast::Iter::List]
    );

    let (generated_ids, generated) =
        al::fresh::exp_from_typ(true, &aliases, &ids(&["bool"]), &nested);
    assert_eq!(generated_ids, ids(&["bool", "bool'"]));
    let il::ast::ExpKind::IterE(inner, (il::ast::Iter::List, outer_binders)) = generated.kind
    else {
        panic!("outer iteration")
    };
    let il::ast::ExpKind::IterE(_, (il::ast::Iter::Opt, inner_binders)) = inner.kind else {
        panic!("inner iteration")
    };
    assert_eq!(inner_binders.len(), 1);
    assert_eq!(outer_binders.len(), 1);
    assert!(inner_binders[0].iters.is_empty());
    assert_eq!(outer_binders[0].iters, vec![il::ast::Iter::Opt]);
}
