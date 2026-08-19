use p4spec_rust::{
    domain::source::{Region, Spanned},
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

fn text(value: &str) -> il::Exp {
    il::Exp::new(
        il::ExpKind::TextE(value.to_owned()),
        il::TypKind::TextT,
        span(value),
    )
}

fn return_text(value: &str, iid: i64) -> sl::Instr {
    sl::Instr::new(sl::InstrKind::ReturnI(text(value)), iid, span(value))
}

fn ambiguous() -> sl::Def {
    Spanned::new(
        sl::DefKind::FuncDecD((
            id("ambiguous"),
            Vec::new(),
            Vec::new(),
            make_type::text_type(),
            vec![return_text("first", 1), return_text("second", 2)],
            None,
            Vec::new(),
        )),
        span("function"),
    )
}

fn interpreter(deterministic: bool) -> Interpreter<NullInterface, NullExtern> {
    Interpreter::new(
        &[ambiguous()],
        Options {
            cache: false,
            deterministic,
            guard: false,
        },
        NullInterface,
        NullExtern,
    )
    .unwrap()
}

#[test]
fn sequential_block_uses_first_result_but_deterministic_block_rejects_ambiguity() {
    let mut sequential = interpreter(false);
    assert_eq!(
        get::text(&sequential.eval_func("ambiguous", &[], &[]).unwrap()),
        Ok("first")
    );

    let mut deterministic = interpreter(true);
    let error = deterministic.eval_func("ambiguous", &[], &[]).unwrap_err();
    assert_eq!(error.span, span("second"));
    assert!(
        error
            .message
            .contains("nondeterministic instruction evaluation")
    );
}
