use p4spec_rust::{
    lang::common::source::{Position, Span},
    lang::{
        common::notation::mixfix::Mixfix, hints::input::InputHint, il, sl, traits::eq::SyntaxEq,
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

fn variable(name: &str) -> il::ast::Exp {
    p4spec_rust::note_phrase! {
        node: il::ast::ExpKind::Var(id(name)),
        note: il::ast::TypKind::Bool,
        span: span(name),
    }
}

fn instruction(kind: sl::ast::InstrKind, iid: i64, source: &str) -> sl::ast::Instr {
    p4spec_rust::note_phrase! {
        node: kind,
        note: iid,
        span: span(source),
    }
}

#[test]
fn instruction_equality_ignores_iids_and_source_regions() {
    let instr_l = instruction(
        sl::ast::InstrKind::Return(sl::ast::ReturnInstr { exp: variable("x") }),
        1,
        "left",
    );
    let instr_r = instruction(
        sl::ast::InstrKind::Return(sl::ast::ReturnInstr { exp: variable("x") }),
        99,
        "right",
    );

    assert!(instr_l.syntax_eq(&instr_r));
}

#[test]
fn subtype_guards_ignore_subcheck_strategy_but_compare_type() {
    let guard_skip = sl::ast::Guard::Sub(typ(), Box::new(il::ast::Subcheck::Skip));
    let guard_recurse = sl::ast::Guard::Sub(typ(), Box::new(il::ast::Subcheck::Recurse(typ())));
    let guard_text = sl::ast::Guard::Sub(
        p4spec_rust::phrase! {
            node: il::ast::TypKind::Text,
            span: span("text"),
        },
        Box::new(il::ast::Subcheck::Skip),
    );

    assert!(guard_skip.syntax_eq(&guard_recurse));
    assert!(!guard_skip.syntax_eq(&guard_text));
}

#[test]
fn rule_instructions_compare_inputs_iterations_and_nested_blocks() {
    let instr_rule = |input_hint| {
        instruction(
            sl::ast::InstrKind::Rule(sl::ast::RuleInstr {
                id: id("relation"),
                not_exp: Mixfix::Arg(variable("x")),
                input_hint,
                iter_instrs: Vec::new(),
                block: vec![instruction(
                    sl::ast::InstrKind::Return(sl::ast::ReturnInstr { exp: variable("x") }),
                    2,
                    "nested",
                )],
            }),
            1,
            "rule",
        )
    };

    assert!(instr_rule(InputHint::new(vec![0])).syntax_eq(&instr_rule(InputHint::new(vec![0]))));
    assert!(!instr_rule(InputHint::new(vec![0])).syntax_eq(&instr_rule(InputHint::new(vec![1]))));
}

#[test]
fn holding_cases_compare_variant_blocks_and_dangling_flags() {
    let block = vec![instruction(
        sl::ast::InstrKind::Return(sl::ast::ReturnInstr { exp: variable("x") }),
        1,
        "block",
    )];

    assert!(
        sl::ast::HoldCase::Hold(block.clone(), false)
            .syntax_eq(&sl::ast::HoldCase::Hold(block.clone(), false))
    );
    assert!(
        !sl::ast::HoldCase::Hold(block.clone(), false)
            .syntax_eq(&sl::ast::HoldCase::Hold(block.clone(), true))
    );
    assert!(
        !sl::ast::HoldCase::Hold(block.clone(), false)
            .syntax_eq(&sl::ast::HoldCase::NotHold(block, false))
    );
}
