use p4spec_rust::{
    domain::{
        mixfix::Mixfix,
        sets::IdSet,
        source::{Position, Span, Spanned},
    },
    lang::{hints::input::InputHint, il, sl},
};

fn span(name: &str) -> Span {
    Span::new(Position::new(name, 0, 0), Position::new(name, 0, 0))
}

fn id(name: &str) -> il::ast::Id {
    Spanned::new(name.to_owned(), span(name))
}

fn typ() -> il::ast::Typ {
    Spanned::new(il::ast::TypKind::Bool, span("type"))
}

fn variable(name: &str) -> il::ast::Exp {
    il::ast::exp(
        il::ast::ExpKind::Var(id(name)),
        il::ast::TypKind::Bool,
        span(name),
    )
}

fn instr(kind: sl::ast::InstrKind) -> sl::ast::Instr {
    sl::ast::instr(kind, 0, span("instruction"))
}

fn names(items: &[&str]) -> IdSet {
    items.iter().map(|item| (*item).to_owned()).collect()
}

#[test]
fn parameters_collect_only_expression_defaults() {
    let expression = Spanned::new(
        sl::ast::ParamKind::Exp(typ(), Box::new(variable("default"))),
        span("expression-parameter"),
    );
    let definition = Spanned::new(
        sl::ast::ParamKind::Def(id("f"), Vec::new(), Vec::new(), typ()),
        span("definition-parameter"),
    );

    assert_eq!(sl::free::free_param(&expression), names(&["default"]));
    assert_eq!(sl::free::free_param(&definition), names(&[]));
}

#[test]
fn guards_collect_only_embedded_expressions() {
    let cases = vec![
        (sl::ast::Guard::Bool(true), names(&[])),
        (
            sl::ast::Guard::Cmp(
                il::ast::CmpOp::Bool(p4spec_rust::lang::xl::bool::CmpOp::Eq),
                il::ast::OpTyp::Bool,
                variable("comparison"),
            ),
            names(&["comparison"]),
        ),
        (
            sl::ast::Guard::Sub(typ(), Box::new(il::ast::Subcheck::Skip)),
            names(&[]),
        ),
        (
            sl::ast::Guard::Match(il::ast::Pattern::List(il::ast::ListPattern::Nil)),
            names(&[]),
        ),
        (sl::ast::Guard::Mem(variable("member")), names(&["member"])),
    ];

    for (guard, expected) in cases {
        assert_eq!(sl::free::free_guard(&guard), expected);
    }
}

#[test]
fn instructions_collect_nested_expressions_and_omit_binding_metadata() {
    let hidden = instr(sl::ast::InstrKind::Return(sl::ast::ReturnInstr {
        exp: variable("hidden"),
    }));
    let binder = il::ast::Var {
        id: id("binder"),
        typ: typ(),
        iters: Vec::new(),
    };
    let signature = sl::ast::RelSignature {
        not_typ: Spanned::new(Mixfix::Seq(Vec::new()), span("notation")),
        input_hint: InputHint::new(vec![0]),
    };
    let instructions = vec![
        (
            instr(sl::ast::InstrKind::If(sl::ast::IfInstr {
                exp: variable("condition"),
                iter_exps: vec![(il::ast::Iter::List, vec![binder.clone()])],
                block: vec![instr(sl::ast::InstrKind::Return(sl::ast::ReturnInstr {
                    exp: variable("then"),
                }))],
                dangle: false,
            })),
            names(&["condition", "then"]),
        ),
        (
            instr(sl::ast::InstrKind::Hold(sl::ast::HoldInstr {
                id: id("relation"),
                not_exp: Mixfix::Arg(variable("hold")),
                iter_exps: vec![(il::ast::Iter::List, vec![binder.clone()])],
                hold_case: sl::ast::HoldCase::Hold(vec![hidden.clone()], false),
            })),
            names(&["hold"]),
        ),
        (
            instr(sl::ast::InstrKind::Case(sl::ast::CaseInstr {
                exp: variable("scrutinee"),
                cases: vec![sl::ast::Case {
                    guard: sl::ast::Guard::Mem(variable("guard")),
                    block: vec![instr(sl::ast::InstrKind::Return(sl::ast::ReturnInstr {
                        exp: variable("arm"),
                    }))],
                }],
                dangle: false,
            })),
            names(&["scrutinee", "guard", "arm"]),
        ),
        (
            instr(sl::ast::InstrKind::Group(sl::ast::GroupInstr {
                id: id("group"),
                rel_signature: signature.clone(),
                exps: vec![variable("group-input")],
                block: vec![instr(sl::ast::InstrKind::Return(sl::ast::ReturnInstr {
                    exp: variable("group-body"),
                }))],
            })),
            names(&["group-input", "group-body"]),
        ),
        (
            instr(sl::ast::InstrKind::Let(sl::ast::LetInstr {
                exp_l: variable("left"),
                exp_r: variable("right"),
                iter_instrs: vec![il::ast::IterPrem {
                    iter: il::ast::Iter::List,
                    vars_bound: vec![binder.clone()],
                    vars_bind: vec![binder.clone()],
                }],
                block: vec![instr(sl::ast::InstrKind::Return(sl::ast::ReturnInstr {
                    exp: variable("let-body"),
                }))],
            })),
            names(&["left", "right", "let-body"]),
        ),
        (
            instr(sl::ast::InstrKind::Rule(sl::ast::RuleInstr {
                id: id("rule"),
                not_exp: Mixfix::Arg(variable("rule-input")),
                input_hint: InputHint::new(vec![0]),
                iter_instrs: vec![il::ast::IterPrem {
                    iter: il::ast::Iter::List,
                    vars_bound: vec![binder.clone()],
                    vars_bind: vec![binder],
                }],
                block: vec![instr(sl::ast::InstrKind::Return(sl::ast::ReturnInstr {
                    exp: variable("rule-body"),
                }))],
            })),
            names(&["rule-input", "rule-body"]),
        ),
        (
            instr(sl::ast::InstrKind::Result(sl::ast::ResultInstr {
                rel_signature: signature,
                exps: vec![variable("result")],
            })),
            names(&["result"]),
        ),
        (
            instr(sl::ast::InstrKind::Debug(sl::ast::DebugInstr {
                exp: variable("debug"),
                instr: Box::new(instr(sl::ast::InstrKind::Return(sl::ast::ReturnInstr {
                    exp: variable("nested"),
                }))),
            })),
            names(&["debug", "nested"]),
        ),
    ];

    for (instruction, expected) in instructions {
        assert_eq!(sl::free::free_instr(&instruction), expected);
    }
}
