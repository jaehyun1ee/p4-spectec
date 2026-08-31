use p4spec_rust::{
    lang::common::source::{Position, Span},
    lang::{common::notation::mixfix::Mixfix, hints::input::InputHint, il, pl},
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

fn exp(kind: pl::ast::ExpKind) -> pl::ast::Exp {
    pl::annot::Annotated {
        node: p4spec_rust::note_phrase! {
            node: kind,
            note: il::ast::TypKind::Bool,
            span: span("expression"),
        },
        hints: pl::annot::Hints::default(),
    }
}

fn variable(name: &str) -> pl::ast::Exp {
    exp(pl::ast::ExpKind::Var(id(name)))
}

fn call(name: &str) -> pl::ast::Exp {
    exp(pl::ast::ExpKind::Call(id(name), Vec::new(), Vec::new()))
}

fn group_instr(
    kind: pl::ast::InstrKind<pl::ast::InstrGroup>,
) -> pl::ast::Instr<pl::ast::InstrGroup> {
    pl::annot::Annotated {
        node: p4spec_rust::note_phrase! { node: kind, note: pl::ast::InstrNote {
            iid: 0,
            fallthrough: None,
        }, span: span("instruction") },
        hints: pl::annot::Hints::default(),
    }
}

#[test]
fn expression_and_path_classification_finds_recursive_calls() {
    let nested = exp(pl::ast::ExpKind::Tuple(vec![exp(pl::ast::ExpKind::Upd(
        Box::new(variable("base")),
        Box::new(p4spec_rust::note_phrase! { node: pl::ast::PathKind::Idx(
        Box::new(p4spec_rust::note_phrase! {
            node: pl::ast::PathKind::Root,
            note: il::ast::TypKind::Bool,
            span: span("root"),
        }),
        Box::new(call("index")),
        ), note: il::ast::TypKind::Bool, span: span("path") }),
        Box::new(variable("field")),
    ))]));

    assert!(pl::partial::is_partial_exp(&nested));
    assert!(!pl::partial::is_partial_exp(&variable("plain")));
}

#[test]
fn guard_classification_preserves_immediate_failure_rules() {
    assert!(pl::partial::is_partial_guard(&pl::ast::Guard::Cmp(
        il::ast::CmpOp::Bool(p4spec_rust::lang::xl::bool::CmpOp::Eq),
        il::ast::OpTyp::Bool,
        call("comparison"),
    )));
    assert!(pl::partial::is_partial_guard(&pl::ast::Guard::CheckLetSub(
        typ(),
        Box::new(il::ast::Subcheck::Skip),
        call("binding"),
    )));
    assert!(!pl::partial::is_partial_guard(&pl::ast::Guard::Mem(call(
        "membership"
    ))));
    assert!(!pl::partial::is_partial_guard(&pl::ast::Guard::Sub(
        typ(),
        Box::new(il::ast::Subcheck::Skip),
    )));
}

#[test]
fn shared_instruction_classification_ignores_nested_block_execution() {
    let nested_call = group_instr(pl::ast::InstrKind::Tier(pl::ast::TierInstr {
        tier: pl::ast::InstrGroup::Return(pl::ast::ReturnGroupInstr {
            exp: call("nested"),
        }),
    }));
    let branch = group_instr(pl::ast::InstrKind::If(pl::ast::IfInstr {
        exp: variable("condition"),
        iter_exps: Vec::new(),
        block: vec![nested_call],
        dangle: false,
    }));
    let hold = group_instr(pl::ast::InstrKind::Hold(pl::ast::HoldInstr {
        id: id("relation"),
        not_exp: Mixfix::Arg(variable("argument")),
        iter_exps: Vec::new(),
        hold_case: pl::ast::HoldCase::Hold(Vec::new(), false),
    }));

    assert!(!pl::partial::is_partial_instr(
        pl::partial::is_partial_instr_group,
        &branch,
    ));
    assert!(pl::partial::is_partial_instr(
        pl::partial::is_partial_instr_group,
        &hold,
    ));
}

#[test]
fn tier_classification_handles_rules_results_backtracking_and_dispatch() {
    assert!(pl::partial::is_partial_instr_group(
        &pl::ast::InstrGroup::Rule(pl::ast::RuleGroupInstr {
            id: id("rule"),
            not_exp: Mixfix::Arg(call("argument")),
            input_hint: InputHint::new(vec![0]),
            iter_instrs: Vec::new(),
        })
    ));
    assert!(pl::partial::is_partial_instr_group(
        &pl::ast::InstrGroup::Result(pl::ast::ResultGroupInstr {
            rel_signature: pl::ast::RelSignature {
                not_typ: p4spec_rust::phrase! {
                    node: Mixfix::Arg(typ()),
                    span: span("signature"),
                },
                input_hint: InputHint::new(Vec::new()),
            },
            exps_output: vec![call("result")],
        })
    ));
    assert!(!pl::partial::is_partial_instr_group(
        &pl::ast::InstrGroup::Backtrack(pl::ast::BacktrackGroupInstr {
            blocks: vec![vec![group_instr(pl::ast::InstrKind::Tier(
                pl::ast::TierInstr {
                    tier: pl::ast::InstrGroup::Return(pl::ast::ReturnGroupInstr {
                        exp: call("arm"),
                    }),
                },
            ))]],
        })
    ));
    assert!(!pl::partial::is_partial_instr_dispatch(
        &pl::ast::InstrDispatch::Route(pl::ast::RouteDispatchInstr { blocks: Vec::new() })
    ));
}
