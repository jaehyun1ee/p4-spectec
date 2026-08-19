use num_bigint::BigInt;
use p4spec_rust::{
    domain::source::{Region, Spanned},
    interface::{NullExtern, NullInterface},
    interp::sl::{Interpreter, Options},
    lang::{il::ast as il, sl::ast as sl, xl::num},
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

fn variable(name: &str) -> il::Exp {
    il::Exp::new(
        il::ExpKind::VarE(id(name)),
        il::TypKind::NumT(num::Typ::IntT),
        span(name),
    )
}

fn integer(value: i64) -> il::Exp {
    il::Exp::new(
        il::ExpKind::NumE(il::Num::Int(BigInt::from(value))),
        il::TypKind::NumT(num::Typ::IntT),
        span("integer"),
    )
}

fn countdown() -> sl::Def {
    let input = variable("n");
    let parameter = Spanned::new(
        sl::ParamKind::ExpP(make_type::int_type(), input.clone()),
        span("parameter"),
    );
    let condition = il::Exp::new(
        il::ExpKind::CmpE(
            il::CmpOp::EqOp,
            il::OpTyp::IntT,
            Box::new(input.clone()),
            Box::new(integer(0)),
        ),
        il::TypKind::BoolT,
        span("condition"),
    );
    let decrement = il::Exp::new(
        il::ExpKind::BinE(
            il::BinOp::SubOp,
            il::OpTyp::IntT,
            Box::new(input),
            Box::new(integer(1)),
        ),
        il::TypKind::NumT(num::Typ::IntT),
        span("decrement"),
    );
    let call = il::Exp::new(
        il::ExpKind::CallE(
            id("countdown"),
            Vec::new(),
            vec![Spanned::new(il::ArgKind::ExpA(decrement), span("argument"))],
        ),
        il::TypKind::NumT(num::Typ::IntT),
        span("call"),
    );
    Spanned::new(
        sl::DefKind::FuncDecD((
            id("countdown"),
            Vec::new(),
            vec![parameter],
            make_type::int_type(),
            vec![
                sl::Instr::new(
                    sl::InstrKind::IfI(
                        condition,
                        Vec::new(),
                        vec![sl::Instr::new(
                            sl::InstrKind::ReturnI(integer(0)),
                            2,
                            span("zero"),
                        )],
                        false,
                    ),
                    1,
                    span("if"),
                ),
                sl::Instr::new(sl::InstrKind::ReturnI(call), 3, span("recurse")),
            ],
            None,
            Vec::new(),
        )),
        span("countdown"),
    )
}

#[test]
fn tail_recursive_function_does_not_grow_the_rust_stack() {
    let mut interpreter = Interpreter::new(
        &[countdown()],
        Options {
            cache: false,
            deterministic: false,
            guard: false,
        },
        NullInterface,
        NullExtern,
    )
    .unwrap();
    let input = make::int(BigInt::from(10_000), span("input"));

    let result = interpreter
        .eval_func("countdown", &[], std::slice::from_ref(&input))
        .unwrap();
    assert_eq!(get::num(&result), Ok(&il::Num::Int(BigInt::from(0))));
}
