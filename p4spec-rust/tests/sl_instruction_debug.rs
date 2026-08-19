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

#[test]
fn debug_instruction_evaluates_its_expression_then_nested_instruction() {
    let debug = sl::Instr::new(
        sl::InstrKind::DebugI(
            text("watched"),
            Box::new(sl::Instr::new(
                sl::InstrKind::ReturnI(text("done")),
                2,
                span("return"),
            )),
        ),
        1,
        span("debug"),
    );
    let function = Spanned::new(
        sl::DefKind::FuncDecD((
            id("debug_then_return"),
            Vec::new(),
            Vec::new(),
            make_type::text_type(),
            vec![debug],
            None,
            Vec::new(),
        )),
        span("function"),
    );
    let mut interpreter = Interpreter::new(
        &[function],
        Options {
            cache: false,
            deterministic: false,
            guard: false,
        },
        NullInterface,
        NullExtern,
    )
    .unwrap();

    assert_eq!(
        get::text(
            &interpreter
                .eval_func("debug_then_return", &[], &[])
                .unwrap()
        ),
        Ok("done")
    );
}
