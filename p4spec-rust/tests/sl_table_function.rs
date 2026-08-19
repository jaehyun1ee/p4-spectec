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

#[test]
fn table_rows_are_flattened_and_always_evaluated_sequentially() {
    let rows = vec![
        (Vec::new(), text("unused"), vec![return_text("first", 1)]),
        (Vec::new(), text("unused"), vec![return_text("second", 2)]),
    ];
    let table = Spanned::new(
        sl::DefKind::TableDecD((
            id("lookup"),
            Vec::new(),
            make_type::text_type(),
            rows,
            Vec::new(),
        )),
        span("table"),
    );
    let mut interpreter = Interpreter::new(
        &[table],
        Options {
            cache: false,
            deterministic: true,
            guard: false,
        },
        NullInterface,
        NullExtern,
    )
    .unwrap();

    assert_eq!(
        get::text(&interpreter.eval_func("lookup", &[], &[]).unwrap()),
        Ok("first")
    );
}
