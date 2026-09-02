use super::*;

#[test]
fn test_free_expression_path_argument_and_premise_variants_collect_identifier_text() {
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
            al::ast::PremKind::Rule(al::ast::RulePrem {
                id: id("r"),
                not_exp: not_exp("x"),
                input_hint: InputHint::new(Vec::new()),
            }),
            ids(&["x"]),
        ),
        (
            al::ast::PremKind::If(al::ast::IfPrem { exp: variable("x") }),
            ids(&["x"]),
        ),
        (
            al::ast::PremKind::IfHold(al::ast::IfHoldPrem {
                id: id("r"),
                not_exp: not_exp("x"),
            }),
            ids(&["x"]),
        ),
        (
            al::ast::PremKind::IfNotHold(al::ast::IfNotHoldPrem {
                id: id("r"),
                not_exp: not_exp("x"),
            }),
            ids(&["x"]),
        ),
        (
            al::ast::PremKind::Let(al::ast::LetPrem {
                exp_l: variable("x"),
                exp_r: variable("y"),
            }),
            ids(&["x", "y"]),
        ),
        (
            al::ast::PremKind::Iter(al::ast::IterPrem {
                prem: Box::new(p4spec_rust::phrase! {
                    node: al::ast::PremKind::If(al::ast::IfPrem { exp: variable("x") }),
                    span: span("nested"),
                }),
                prem_iter: il::ast::PremIter {
                    iter: il::ast::Iter::List,
                    vars_bound: Vec::new(),
                    vars_bind: Vec::new(),
                },
            }),
            ids(&["x"]),
        ),
        (
            al::ast::PremKind::Debug(al::ast::DebugPrem { exp: variable("x") }),
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
fn test_free_al_shapes_and_definition_arms_are_exhaustive() {
    let premise = || {
        p4spec_rust::phrase! {
            node: al::ast::PremKind::If(al::ast::IfPrem { exp: variable("p") }),
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
    let clause: al::ast::Clause = p4spec_rust::phrase! { node: al::ast::ClauseKind {
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
