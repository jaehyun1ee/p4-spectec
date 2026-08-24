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

fn instr(kind: sl::ast::InstrKind) -> sl::ast::Instr {
    sl::ast::Instr::new(kind, 0, span("instruction"))
}

fn names(items: &[&str]) -> sl::free::FreeVars {
    items.iter().map(|item| (*item).to_owned()).collect()
}

#[test]
fn parameters_collect_only_expression_defaults() {
    let expression = Spanned::new(
        sl::ast::ParamKind::ExpP(typ(), Box::new(variable("default"))),
        span("expression-parameter"),
    );
    let definition = Spanned::new(
        sl::ast::ParamKind::DefP(id("f"), Vec::new(), Vec::new(), typ()),
        span("definition-parameter"),
    );

    assert_eq!(sl::free::free_param(&expression), names(&["default"]));
    assert_eq!(sl::free::free_param(&definition), names(&[]));
}

#[test]
fn guards_collect_only_embedded_expressions() {
    let cases = vec![
        (sl::ast::Guard::BoolG(true), names(&[])),
        (
            sl::ast::Guard::CmpG(
                il::ast::CmpOp::EqOp,
                il::ast::OpTyp::BoolT,
                variable("comparison"),
            ),
            names(&["comparison"]),
        ),
        (
            sl::ast::Guard::SubG(typ(), Box::new(il::ast::Subcheck::SkipSC)),
            names(&[]),
        ),
        (
            sl::ast::Guard::MatchG(il::ast::Pattern::ListP(il::ast::ListPattern::Nil)),
            names(&[]),
        ),
        (sl::ast::Guard::MemG(variable("member")), names(&["member"])),
    ];

    for (guard, expected) in cases {
        assert_eq!(sl::free::free_guard(&guard), expected);
    }
}

#[test]
fn instructions_collect_nested_expressions_and_omit_binding_metadata() {
    let hidden = instr(sl::ast::InstrKind::ReturnI(variable("hidden")));
    let binder = il::ast::Var {
        id: id("binder"),
        typ: typ(),
        iters: Vec::new(),
    };
    let signature = sl::ast::RelSignature {
        notation: Spanned::new(Mixfix::Seq(Vec::new()), span("notation")),
        input_hint: InputHint::new(vec![0]),
    };
    let instructions = vec![
        (
            instr(sl::ast::InstrKind::IfI(
                variable("condition"),
                vec![(il::ast::Iter::List, vec![binder.clone()])],
                vec![instr(sl::ast::InstrKind::ReturnI(variable("then")))],
                false,
            )),
            names(&["condition", "then"]),
        ),
        (
            instr(sl::ast::InstrKind::HoldI(
                id("relation"),
                Mixfix::Arg(variable("hold")),
                vec![(il::ast::Iter::List, vec![binder.clone()])],
                sl::ast::HoldCase::HoldH(vec![hidden.clone()], false),
            )),
            names(&["hold"]),
        ),
        (
            instr(sl::ast::InstrKind::CaseI(
                variable("scrutinee"),
                vec![sl::ast::Case {
                    guard: sl::ast::Guard::MemG(variable("guard")),
                    block: vec![instr(sl::ast::InstrKind::ReturnI(variable("arm")))],
                }],
                false,
            )),
            names(&["scrutinee", "guard", "arm"]),
        ),
        (
            instr(sl::ast::InstrKind::GroupI(
                id("group"),
                signature.clone(),
                vec![variable("group-input")],
                vec![instr(sl::ast::InstrKind::ReturnI(variable("group-body")))],
            )),
            names(&["group-input", "group-body"]),
        ),
        (
            instr(sl::ast::InstrKind::LetI(
                variable("left"),
                variable("right"),
                vec![il::ast::IterPrem {
                    iter: il::ast::Iter::List,
                    vars_bound: vec![binder.clone()],
                    vars_bind: vec![binder.clone()],
                }],
                vec![instr(sl::ast::InstrKind::ReturnI(variable("let-body")))],
            )),
            names(&["left", "right", "let-body"]),
        ),
        (
            instr(sl::ast::InstrKind::RuleI(
                id("rule"),
                Mixfix::Arg(variable("rule-input")),
                InputHint::new(vec![0]),
                vec![il::ast::IterPrem {
                    iter: il::ast::Iter::List,
                    vars_bound: vec![binder.clone()],
                    vars_bind: vec![binder],
                }],
                vec![instr(sl::ast::InstrKind::ReturnI(variable("rule-body")))],
            )),
            names(&["rule-input", "rule-body"]),
        ),
        (
            instr(sl::ast::InstrKind::ResultI(
                signature,
                vec![variable("result")],
            )),
            names(&["result"]),
        ),
        (
            instr(sl::ast::InstrKind::DebugI(
                variable("debug"),
                Box::new(instr(sl::ast::InstrKind::ReturnI(variable("nested")))),
            )),
            names(&["debug", "nested"]),
        ),
    ];

    for (instruction, expected) in instructions {
        assert_eq!(sl::free::free_instr(&instruction), expected);
    }
}
