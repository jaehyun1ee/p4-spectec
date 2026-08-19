use num_bigint::BigInt;
use p4spec_rust::{
    domain::{
        mixfix::Mixfix,
        source::{Region, Spanned},
    },
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
    variable_at(name, name)
}

fn variable_at(name: &str, source: &str) -> il::Exp {
    il::Exp::new(
        il::ExpKind::VarE(Spanned::new(name.to_owned(), span(source))),
        il::TypKind::NumT(num::Typ::IntT),
        span(source),
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

fn signature() -> sl::RelSignature {
    (
        Spanned::new(Mixfix::Seq(Vec::new()), span("signature")),
        Vec::new(),
    )
}

fn countdown_relation() -> sl::Def {
    let input = variable("n");
    let output_notation = variable_at("output", "output-notation");
    let output_result = variable_at("output", "output-result");
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
    let rule = sl::Instr::new(
        sl::InstrKind::RuleI(
            id("countdown_relation"),
            Mixfix::Seq(vec![Mixfix::Arg(decrement), Mixfix::Arg(output_notation)]),
            vec![0],
            Vec::new(),
            vec![sl::Instr::new(
                sl::InstrKind::ResultI(signature(), vec![output_result]),
                6,
                span("result-recursive"),
            )],
        ),
        5,
        span("rule"),
    );
    Spanned::new(
        sl::DefKind::RelD((
            id("countdown_relation"),
            signature(),
            vec![variable("n")],
            vec![
                sl::Instr::new(
                    sl::InstrKind::IfI(
                        condition,
                        Vec::new(),
                        vec![sl::Instr::new(
                            sl::InstrKind::ResultI(signature(), vec![integer(0)]),
                            4,
                            span("result-zero"),
                        )],
                        false,
                    ),
                    3,
                    span("if"),
                ),
                rule,
            ],
            None,
            Vec::new(),
        )),
        span("countdown-relation"),
    )
}

fn tail_position_fallbacks() -> [sl::Def; 2] {
    let tuple_pattern = il::Exp::new(
        il::ExpKind::TupleE(vec![variable("item")]),
        il::TypKind::TupleT(vec![make_type::int_type()]),
        span("tuple-pattern"),
    );
    let tuple_only = Spanned::new(
        sl::DefKind::FuncDecD((
            id("tuple_only"),
            Vec::new(),
            vec![Spanned::new(
                sl::ParamKind::ExpP(
                    make_type::tuple_type(vec![make_type::int_type()]),
                    tuple_pattern,
                ),
                span("parameter"),
            )],
            make_type::int_type(),
            vec![sl::Instr::new(
                sl::InstrKind::ReturnI(integer(1)),
                7,
                span("tuple-return"),
            )],
            None,
            Vec::new(),
        )),
        span("tuple-only"),
    );
    let failed_call = il::Exp::new(
        il::ExpKind::CallE(
            id("tuple_only"),
            Vec::new(),
            vec![Spanned::new(
                il::ArgKind::ExpA(integer(7)),
                span("argument"),
            )],
        ),
        il::TypKind::NumT(num::Typ::IntT),
        span("failed-call"),
    );
    let fallback = Spanned::new(
        sl::DefKind::FuncDecD((
            id("fallback"),
            Vec::new(),
            Vec::new(),
            make_type::int_type(),
            vec![
                sl::Instr::new(sl::InstrKind::ReturnI(failed_call), 8, span("try-call")),
                sl::Instr::new(sl::InstrKind::ReturnI(integer(42)), 9, span("fallback")),
            ],
            None,
            Vec::new(),
        )),
        span("fallback-function"),
    );
    [tuple_only, fallback]
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

#[test]
fn tail_recursive_relation_does_not_grow_the_rust_stack() {
    let mut interpreter = Interpreter::new(
        &[countdown_relation()],
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
        .eval_rel("countdown_relation", std::slice::from_ref(&input))
        .unwrap();
    assert_eq!(result.len(), 1);
    assert_eq!(get::num(&result[0]), Ok(&il::Num::Int(BigInt::from(0))));
}

#[test]
fn failed_call_before_the_tail_position_continues_to_the_fallback() {
    let mut interpreter = Interpreter::new(
        &tail_position_fallbacks(),
        Options {
            cache: false,
            deterministic: false,
            guard: false,
        },
        NullInterface,
        NullExtern,
    )
    .unwrap();

    let result = interpreter.eval_func("fallback", &[], &[]).unwrap();
    assert_eq!(get::num(&result), Ok(&il::Num::Int(BigInt::from(42))));
}
