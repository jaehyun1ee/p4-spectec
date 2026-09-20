use p4spec_rust::{
    lang::common::source::{Position, Span},
    lang::{common::notation::mixfix::Mixfix, hints::input::InputHint, il, pl},
};

fn span(name: &str) -> Span {
    Span::new(Position::new(name, 0, 0), Position::new(name, 0, 0))
}

fn id(name: &str) -> il::ast::Id {
    p4spec_rust::phrase! { node: name.to_owned(), span: span(name) }
}

fn typ() -> il::ast::Typ {
    p4spec_rust::phrase! { node: il::ast::TypKind::Bool, span: span("type") }
}

fn exp(exp_kind: pl::ast::ExpKind) -> pl::ast::Exp {
    pl::annot::Annotated {
        node: p4spec_rust::note_phrase! {
            node: exp_kind,
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
    instr_kind: pl::ast::InstrKind<pl::ast::GroupInstr>,
) -> pl::ast::Instr<pl::ast::GroupInstr> {
    pl::annot::Annotated {
        node: p4spec_rust::note_phrase! {
            node: instr_kind,
            note: None,
            span: span("instruction"),
        },
        hints: pl::annot::Hints::default(),
    }
}

#[test]
fn test_expression_and_path_classification_finds_recursive_calls() {
    let exp_nested = exp(pl::ast::ExpKind::Tuple(vec![exp(pl::ast::ExpKind::Upd(
        Box::new(variable("base")),
        Box::new(p4spec_rust::note_phrase! {
            node: pl::ast::PathKind::Idx(
                Box::new(p4spec_rust::note_phrase! {
                    node: pl::ast::PathKind::Root,
                    note: il::ast::TypKind::Bool,
                    span: span("root"),
                }),
                Box::new(call("index")),
            ),
            note: il::ast::TypKind::Bool,
            span: span("path"),
        }),
        Box::new(variable("field")),
    ))]));

    assert!(pl::partial::is_partial_exp(&exp_nested));
    assert!(!pl::partial::is_partial_exp(&variable("plain")));
}

#[test]
fn test_guard_classification_checks_only_evaluated_expressions() {
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
    assert!(!pl::partial::is_partial_guard(&pl::ast::Guard::Mem(call("membership"))));
}

#[test]
fn test_instruction_classification_distinguishes_leaf_and_nested_partiality() {
    let instr_nested_call = group_instr(pl::ast::InstrKind::Tier(pl::ast::TierInstr {
        tier: pl::ast::GroupInstr::Return(pl::ast::ReturnInstr { exp: call("nested") }),
    }));
    let instr_branch = group_instr(pl::ast::InstrKind::If(pl::ast::IfInstr {
        exp: variable("condition"),
        iter_exps: Vec::new(),
        block: vec![instr_nested_call],
        dangle: false,
    }));
    let instr_shorthand = group_instr(pl::ast::InstrKind::OptionGet(pl::ast::OptionGetInstr {
        exp_l: variable("left"),
        exp_r: variable("right"),
        block: Vec::new(),
    }));

    assert!(!pl::partial::is_partial_instr(pl::partial::is_partial_group_instr, &instr_branch,));
    assert!(pl::partial::is_partial_instr(pl::partial::is_partial_group_instr, &instr_shorthand,));
}

#[test]
fn test_tier_classification_uses_rule_arguments_not_rule_kind() {
    assert!(!pl::partial::is_partial_group_instr(&pl::ast::GroupInstr::Rule(pl::ast::RuleInstr {
        id: id("rule"),
        not_exp: Mixfix::Arg(variable("argument")),
        input_hint: InputHint::new(vec![0]),
        iter_instrs: Vec::new(),
    })));
    assert!(pl::partial::is_partial_group_instr(&pl::ast::GroupInstr::Rule(pl::ast::RuleInstr {
        id: id("rule"),
        not_exp: Mixfix::Arg(call("argument")),
        input_hint: InputHint::new(vec![0]),
        iter_instrs: Vec::new(),
    })));
    assert!(!pl::partial::is_partial_group_instr(&pl::ast::GroupInstr::Backtrack(
        pl::ast::BacktrackInstr { blocks: vec![Vec::new()] }
    )));
}
