use p4spec_rust::{
    domain::{
        mixfix::Mixfix,
        source::{Region, Spanned},
    },
    lang::{hints::input::InputHint, il, sl},
};

fn span(name: &str) -> Region {
    Region::for_file(name)
}

fn id(name: &str) -> il::ast::Id {
    Spanned::new(name.to_owned(), span(name))
}

fn typ() -> il::ast::Typ {
    Spanned::new(il::ast::TypKind::BoolT, span("type"))
}

fn variable(name: &str) -> il::ast::Exp {
    il::ast::Exp::new(
        il::ast::ExpKind::VarE(id(name)),
        il::ast::TypKind::BoolT,
        span(name),
    )
}

fn instruction(kind: sl::ast::InstrKind, iid: i64, source: &str) -> sl::ast::Instr {
    sl::ast::Instr::new(kind, iid, span(source))
}

#[test]
fn instruction_equality_ignores_iids_and_source_regions() {
    let left = instruction(sl::ast::InstrKind::ReturnI(variable("x")), 1, "left");
    let right = instruction(sl::ast::InstrKind::ReturnI(variable("x")), 99, "right");

    assert!(sl::eq::eq_instr(&left, &right));
}

#[test]
fn subtype_guards_ignore_subcheck_strategy_but_compare_type() {
    let skip = sl::ast::Guard::SubG(typ(), Box::new(il::ast::Subcheck::SkipSC));
    let recurse = sl::ast::Guard::SubG(typ(), Box::new(il::ast::Subcheck::RecurseSC(typ())));
    let text = sl::ast::Guard::SubG(
        Spanned::new(il::ast::TypKind::TextT, span("text")),
        Box::new(il::ast::Subcheck::SkipSC),
    );

    assert!(sl::eq::eq_guard(&skip, &recurse));
    assert!(!sl::eq::eq_guard(&skip, &text));
}

#[test]
fn rule_instructions_compare_inputs_iterations_and_nested_blocks() {
    let rule = |inputs| {
        instruction(
            sl::ast::InstrKind::RuleI(
                id("relation"),
                Mixfix::Arg(variable("x")),
                inputs,
                Vec::new(),
                vec![instruction(
                    sl::ast::InstrKind::ReturnI(variable("x")),
                    2,
                    "nested",
                )],
            ),
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
        sl::ast::InstrKind::ReturnI(variable("x")),
        1,
        "block",
    )];

    assert!(sl::eq::eq_holdcase(
        &sl::ast::HoldCase::HoldH(block.clone(), false),
        &sl::ast::HoldCase::HoldH(block.clone(), false),
    ));
    assert!(!sl::eq::eq_holdcase(
        &sl::ast::HoldCase::HoldH(block.clone(), false),
        &sl::ast::HoldCase::HoldH(block.clone(), true),
    ));
    assert!(!sl::eq::eq_holdcase(
        &sl::ast::HoldCase::HoldH(block.clone(), false),
        &sl::ast::HoldCase::NotHoldH(block, false),
    ));
}
