use p4spec_rust::{
    domain::{
        mixfix::Mixfix,
        source::{Region, Spanned},
    },
    interface::{NullExtern, NullInterface},
    interp::sl::{Interpreter, Options},
    lang::{il::ast as il, sl::ast as sl},
    runtime::{r#type::typ::make as make_type, value::get},
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
