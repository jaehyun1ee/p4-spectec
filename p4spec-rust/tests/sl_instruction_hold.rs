use p4spec_rust::{
    domain::{
        mixfix::Mixfix,
        source::{Region, Spanned},
    },
    interface::{NullExtern, NullInterface},
    interp::sl::{Interpreter, Options},
    lang::{il::ast as il, sl::ast as sl},
    runtime::{
        r#type::typ::make as make_type,
        value::{get, make},
    },
};

fn span(file: &str) -> Region {
    Region::for_file(file)
}

fn id(name: &str) -> il::Id {
    Spanned::new(name.to_owned(), span(name))
}

fn signature() -> sl::RelSignature {
    (
        Spanned::new(Mixfix::Seq(Vec::new()), span("signature")),
        Vec::new(),
    )
}

fn relation(name: &str, succeeds: bool) -> sl::Def {
    let block = if succeeds {
        vec![sl::Instr::new(
            sl::InstrKind::ResultI(signature(), Vec::new()),
            1,
            span("result"),
        )]
    } else {
        Vec::new()
    };
    Spanned::new(
        sl::DefKind::RelD((id(name), signature(), Vec::new(), block, None, Vec::new())),
        span("relation"),
    )
}

fn text(value: &str) -> il::Exp {
    il::Exp::new(
        il::ExpKind::TextE(value.to_owned()),
        il::TypKind::TextT,
        span("text"),
    )
}

fn variable(name: &str) -> il::Exp {
    il::Exp::new(il::ExpKind::VarE(id(name)), il::TypKind::TextT, span(name))
}

fn return_text(value: &str, iid: i64) -> sl::Instr {
    sl::Instr::new(sl::InstrKind::ReturnI(text(value)), iid, span("return"))
}

fn function(name: &str, block: sl::Block) -> sl::Def {
    Spanned::new(
        sl::DefKind::FuncDecD((
            id(name),
            Vec::new(),
            Vec::new(),
            make_type::text_type(),
            block,
            None,
            Vec::new(),
        )),
        span("function"),
    )
}

fn hold(relation: &str, hold_case: sl::HoldCase, iid: i64) -> sl::Instr {
    sl::Instr::new(
        sl::InstrKind::HoldI(id(relation), Mixfix::Seq(Vec::new()), Vec::new(), hold_case),
        iid,
        span("hold"),
    )
}

fn all_are_yes() -> [sl::Def; 2] {
    let candidate = variable("candidate");
    let condition = il::Exp::new(
        il::ExpKind::CmpE(
            il::CmpOp::EqOp,
            il::OpTyp::BoolT,
            Box::new(candidate.clone()),
            Box::new(text("yes")),
        ),
        il::TypKind::BoolT,
        span("condition"),
    );
    let only_yes = Spanned::new(
        sl::DefKind::RelD((
            id("only_yes"),
            signature(),
            vec![candidate],
            vec![sl::Instr::new(
                sl::InstrKind::IfI(
                    condition,
                    Vec::new(),
                    vec![sl::Instr::new(
                        sl::InstrKind::ResultI(signature(), Vec::new()),
                        14,
                        span("result"),
                    )],
                    false,
                ),
                14,
                span("if"),
            )],
            None,
            Vec::new(),
        )),
        span("only-yes"),
    );
    let text_type = make_type::text_type();
    let list_type = make_type::list_type(text_type.clone());
    let item = (id("item"), text_type, Vec::new());
    let parameter_pattern = il::Exp::new(
        il::ExpKind::IterE(
            Box::new(variable("item")),
            (il::Iter::List, vec![item.clone()]),
        ),
        list_type.node.clone(),
        span("parameter-pattern"),
    );
    let parameter = Spanned::new(
        sl::ParamKind::ExpP(list_type, parameter_pattern),
        span("parameter"),
    );
    let hold = sl::Instr::new(
        sl::InstrKind::HoldI(
            id("only_yes"),
            Mixfix::Seq(vec![Mixfix::Arg(variable("item"))]),
            vec![(il::Iter::List, vec![item])],
            sl::HoldCase::BothH(
                vec![return_text("all", 16)],
                vec![return_text("not-all", 17)],
            ),
        ),
        15,
        span("hold"),
    );
    let function = Spanned::new(
        sl::DefKind::FuncDecD((
            id("all_are_yes"),
            Vec::new(),
            vec![parameter],
            make_type::text_type(),
            vec![hold],
            None,
            Vec::new(),
        )),
        span("all-are-yes"),
    );
    [only_yes, function]
}

#[test]
fn hold_instruction_selects_expected_blocks_and_continues_when_not_met() {
    let both_success = function(
        "both_success",
        vec![hold(
            "succeeds",
            sl::HoldCase::BothH(vec![return_text("hold", 3)], vec![return_text("not", 4)]),
            2,
        )],
    );
    let both_failure = function(
        "both_failure",
        vec![hold(
            "fails",
            sl::HoldCase::BothH(vec![return_text("hold", 6)], vec![return_text("not", 7)]),
            5,
        )],
    );
    let required_failure = function(
        "required_failure",
        vec![
            hold(
                "fails",
                sl::HoldCase::HoldH(vec![return_text("wrong", 9)], false),
                8,
            ),
            return_text("fallback", 10),
        ],
    );
    let forbidden_success = function(
        "forbidden_success",
        vec![
            hold(
                "succeeds",
                sl::HoldCase::NotHoldH(vec![return_text("wrong", 12)], false),
                11,
            ),
            return_text("fallback", 13),
        ],
    );
    let spec = [
        relation("succeeds", true),
        relation("fails", false),
        both_success,
        both_failure,
        required_failure,
        forbidden_success,
    ];
    let mut interpreter = Interpreter::new(
        &spec,
        Options {
            cache: false,
            deterministic: false,
            guard: false,
        },
        NullInterface,
        NullExtern,
    )
    .unwrap();

    for (name, expected) in [
        ("both_success", "hold"),
        ("both_failure", "not"),
        ("required_failure", "fallback"),
        ("forbidden_success", "fallback"),
    ] {
        assert_eq!(
            get::text(&interpreter.eval_func(name, &[], &[]).unwrap()),
            Ok(expected)
        );
    }
}

#[test]
fn iterated_hold_requires_the_relation_for_every_list_element() {
    let mut interpreter = Interpreter::new(
        &all_are_yes(),
        Options {
            cache: false,
            deterministic: false,
            guard: false,
        },
        NullInterface,
        NullExtern,
    )
    .unwrap();
    let list_type = make_type::list_type(make_type::text_type());

    for (items, expected) in [
        (vec!["yes", "yes"], "all"),
        (vec!["yes", "no"], "not-all"),
        (Vec::new(), "all"),
    ] {
        let input = make::list(
            &list_type,
            items
                .into_iter()
                .map(|item| make::text(item.to_owned(), span("item")))
                .collect(),
            span("input"),
        );
        assert_eq!(
            get::text(
                &interpreter
                    .eval_func("all_are_yes", &[], std::slice::from_ref(&input))
                    .unwrap()
            ),
            Ok(expected)
        );
    }
}
