use p4spec_rust::{
    lang::common::source::{Position, Span},
    lang::{
        common::{ds::set::IdSet, notation::mixfix::Mixfix},
        hints::input::InputHint,
        il, sl,
        traits::free::FreeIds,
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

fn id_exp(name: &str) -> il::ast::Exp {
    p4spec_rust::note_phrase!(node: il::ast::ExpKind::Id(id(name)), note: il::ast::TypKind::Bool, span: span(name))
}

fn instr(kind: sl::ast::InstrKind) -> sl::ast::Instr {
    p4spec_rust::phrase! {
        node: kind,
        span: span("instruction"),
    }
}

fn names(items: &[&str]) -> IdSet {
    items.iter().map(|item| id(item)).collect()
}

#[test]
fn test_parameters_collect_only_expression_defaults() {
    let param_exp = p4spec_rust::phrase! {
        node: sl::ast::ParamKind::Exp(typ(), Box::new(id_exp("default"))),
        span: span("expression-parameter"),
    };
    let param_def = p4spec_rust::phrase! {
        node: sl::ast::ParamKind::Def(id("f"), Vec::new(), Vec::new(), typ()),
        span: span("definition-parameter"),
    };

    assert_eq!(param_exp.free_ids(), names(&["default"]));
    assert_eq!(param_def.free_ids(), names(&[]));
}

#[test]
fn test_guards_collect_only_embedded_expressions() {
    let cases = vec![
        (sl::ast::Guard::Bool(true), names(&[])),
        (
            sl::ast::Guard::Cmp(
                il::ast::CmpOp::Bool(p4spec_rust::lang::common::prim::bool::CmpOp::Eq),
                il::ast::OpTyp::Bool,
                id_exp("comparison"),
            ),
            names(&["comparison"]),
        ),
        (sl::ast::Guard::Sub(typ(), Box::new(il::ast::Subcheck::Skip)), names(&[])),
        (sl::ast::Guard::Match(il::ast::Pattern::List(il::ast::ListPattern::Nil)), names(&[])),
        (sl::ast::Guard::Mem(id_exp("member")), names(&["member"])),
    ];

    for (guard, expected) in cases {
        assert_eq!(guard.free_ids(), expected);
    }
}

#[test]
fn test_instructions_collect_nested_expressions_and_omit_binding_metadata() {
    let hidden = instr(sl::ast::InstrKind::Return(sl::ast::ReturnInstr { exp: id_exp("hidden") }));
    let binder = il::ast::Var { id: id("binder"), typ: typ(), iters: Vec::new() };
    let signature = sl::ast::RelSignature {
        not_typ: p4spec_rust::phrase! {
            node: Mixfix::Seq(Vec::new()),
            span: span("notation"),
        },
        input_hint: InputHint::new(vec![0]),
    };
    let instructions = vec![
        (
            instr(sl::ast::InstrKind::If(sl::ast::IfInstr {
                exp: id_exp("condition"),
                iter_exps: vec![il::ast::ExpIter {
                    iter: il::ast::Iter::List,
                    vars: vec![binder.clone()],
                }],
                block: vec![instr(sl::ast::InstrKind::Return(sl::ast::ReturnInstr {
                    exp: id_exp("then"),
                }))],
                dangle: false,
            })),
            names(&["condition", "then"]),
        ),
        (
            instr(sl::ast::InstrKind::Hold(sl::ast::HoldInstr {
                id: id("relation"),
                not_exp: Mixfix::Arg(id_exp("hold")),
                iter_exps: vec![il::ast::ExpIter {
                    iter: il::ast::Iter::List,
                    vars: vec![binder.clone()],
                }],
                hold_case: sl::ast::HoldCase::Hold(vec![hidden.clone()], false),
            })),
            names(&["hold"]),
        ),
        (
            instr(sl::ast::InstrKind::Case(sl::ast::CaseInstr {
                exp: id_exp("scrutinee"),
                cases: vec![sl::ast::Case {
                    guard: sl::ast::Guard::Mem(id_exp("guard")),
                    block: vec![instr(sl::ast::InstrKind::Return(sl::ast::ReturnInstr {
                        exp: id_exp("arm"),
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
                exps: vec![id_exp("group-input")],
                block: vec![instr(sl::ast::InstrKind::Return(sl::ast::ReturnInstr {
                    exp: id_exp("group-body"),
                }))],
            })),
            names(&["group-input", "group-body"]),
        ),
        (
            instr(sl::ast::InstrKind::Let(sl::ast::LetInstr {
                exp_l: id_exp("left"),
                exp_r: id_exp("right"),
                iter_instrs: vec![il::ast::PremIter {
                    iter: il::ast::Iter::List,
                    vars_bound: vec![binder.clone()],
                    vars_bind: vec![binder.clone()],
                }],
                block: vec![instr(sl::ast::InstrKind::Return(sl::ast::ReturnInstr {
                    exp: id_exp("let-body"),
                }))],
            })),
            names(&["left", "right", "let-body"]),
        ),
        (
            instr(sl::ast::InstrKind::Rule(sl::ast::RuleInstr {
                id: id("rule"),
                not_exp: Mixfix::Arg(id_exp("rule-input")),
                input_hint: InputHint::new(vec![0]),
                iter_instrs: vec![il::ast::PremIter {
                    iter: il::ast::Iter::List,
                    vars_bound: vec![binder.clone()],
                    vars_bind: vec![binder],
                }],
                block: vec![instr(sl::ast::InstrKind::Return(sl::ast::ReturnInstr {
                    exp: id_exp("rule-body"),
                }))],
            })),
            names(&["rule-input", "rule-body"]),
        ),
        (
            instr(sl::ast::InstrKind::Result(sl::ast::ResultInstr {
                rel_signature: signature,
                exps: vec![id_exp("result")],
            })),
            names(&["result"]),
        ),
        (
            instr(sl::ast::InstrKind::Debug(sl::ast::DebugInstr {
                exp: id_exp("debug"),
                instr: Box::new(instr(sl::ast::InstrKind::Return(sl::ast::ReturnInstr {
                    exp: id_exp("nested"),
                }))),
            })),
            names(&["debug", "nested"]),
        ),
    ];

    for (instruction, expected) in instructions {
        assert_eq!(instruction.free_ids(), expected);
    }
}
