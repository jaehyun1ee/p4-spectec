use p4spec_rust::{
    lang::common::source::{Position, Span},
    lang::{
        al,
        common::{
            ds::set::IdSet,
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

fn premise(kind: al::ast::PremKind) -> al::ast::Prem {
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
        prems: vec![premise(al::ast::PremKind::If(al::ast::IfPrem {
            exp: variable("ready"),
        }))],
    };
    let evaluate_path = al::ast::RulePath {
        id: id("success"),
        prems: vec![premise(al::ast::PremKind::Debug(al::ast::DebugPrem {
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
        prems: vec![premise(al::ast::PremKind::If(al::ast::IfPrem {
            exp: variable("ready"),
        }))],
    }, span: span(metadata) };
    let function_clause = p4spec_rust::phrase! { node: al::ast::ClauseKind {
        args: vec![arg_exp("argument")],
        expression: text_expression("quoted\"\\"),
        premises: vec![premise(al::ast::PremKind::If(al::ast::IfPrem {
            exp: variable("ready"),
        }))],
    }, span: span(metadata) };
    let else_clause = p4spec_rust::phrase! { node: al::ast::ClauseKind {
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

#[path = "al/eq.rs"]
mod eq;
#[path = "al/free.rs"]
mod free;
#[path = "al/print.rs"]
mod print;
