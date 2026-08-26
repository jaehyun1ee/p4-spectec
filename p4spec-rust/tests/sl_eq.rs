use p4spec_rust::{
    domain::{
        mixfix::Mixfix,
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

fn instruction(kind: sl::ast::InstrKind, iid: i64, source: &str) -> sl::ast::Instr {
    sl::ast::instr(kind, iid, span(source))
}

#[test]
fn instruction_equality_ignores_iids_and_source_regions() {
    let left = instruction(
        sl::ast::InstrKind::Return(sl::ast::ReturnInstr { exp: variable("x") }),
        1,
        "left",
    );
    let right = instruction(
        sl::ast::InstrKind::Return(sl::ast::ReturnInstr { exp: variable("x") }),
        99,
        "right",
    );

    assert!(sl::eq::eq_instr(&left, &right));
}

#[test]
fn subtype_guards_ignore_subcheck_strategy_but_compare_type() {
    let skip = sl::ast::Guard::Sub(typ(), Box::new(il::ast::Subcheck::Skip));
    let recurse = sl::ast::Guard::Sub(typ(), Box::new(il::ast::Subcheck::Recurse(typ())));
    let text = sl::ast::Guard::Sub(
        Spanned::new(il::ast::TypKind::Text, span("text")),
        Box::new(il::ast::Subcheck::Skip),
    );

    assert!(sl::eq::eq_guard(&skip, &recurse));
    assert!(!sl::eq::eq_guard(&skip, &text));
}

#[test]
fn rule_instructions_compare_inputs_iterations_and_nested_blocks() {
    let rule = |inputs| {
        instruction(
            sl::ast::InstrKind::Rule(sl::ast::RuleInstr {
                id: id("relation"),
                not_exp: Mixfix::Arg(variable("x")),
                input_hint: inputs,
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

    assert!(sl::eq::eq_instr(
        &rule(InputHint::new(vec![0])),
        &rule(InputHint::new(vec![0]))
    ));
    assert!(!sl::eq::eq_instr(
        &rule(InputHint::new(vec![0])),
        &rule(InputHint::new(vec![1]))
    ));
}

#[test]
fn holding_cases_compare_variant_blocks_and_dangling_flags() {
    let block = vec![instruction(
        sl::ast::InstrKind::Return(sl::ast::ReturnInstr { exp: variable("x") }),
        1,
        "block",
    )];

    assert!(sl::eq::eq_holdcase(
        &sl::ast::HoldCase::Hold(block.clone(), false),
        &sl::ast::HoldCase::Hold(block.clone(), false),
    ));
    assert!(!sl::eq::eq_holdcase(
        &sl::ast::HoldCase::Hold(block.clone(), false),
        &sl::ast::HoldCase::Hold(block.clone(), true),
    ));
    assert!(!sl::eq::eq_holdcase(
        &sl::ast::HoldCase::Hold(block.clone(), false),
        &sl::ast::HoldCase::NotHold(block, false),
    ));
}
