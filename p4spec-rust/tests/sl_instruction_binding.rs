use p4spec_rust::{
    domain::source::{Region, Spanned},
    interface::{BuiltinInterface, NullExtern},
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

fn variable(name: &str) -> il::Exp {
    il::Exp::new(il::ExpKind::VarE(id(name)), il::TypKind::TextT, span(name))
}

fn text(value: &str) -> il::Exp {
    il::Exp::new(
        il::ExpKind::TextE(value.to_owned()),
        il::TypKind::TextT,
        span("text"),
    )
}

fn return_exp(exp: il::Exp, iid: i64) -> sl::Instr {
    sl::Instr::new(sl::InstrKind::ReturnI(exp), iid, span("return"))
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

fn interpreter(spec: &[sl::Def]) -> Interpreter<BuiltinInterface, NullExtern> {
    Interpreter::new(
        spec,
        Options {
            cache: false,
            deterministic: false,
            guard: false,
        },
        BuiltinInterface::new(),
        NullExtern,
    )
    .unwrap()
}

#[test]
fn let_instruction_binds_only_inside_its_nested_block() {
    let bind = function(
        "bind",
        vec![sl::Instr::new(
            sl::InstrKind::LetI(
                variable("x"),
                text("bound"),
                Vec::new(),
                vec![return_exp(variable("x"), 2)],
            ),
            1,
            span("let"),
        )],
    );
    let no_leak = function(
        "no_leak",
        vec![
            sl::Instr::new(
                sl::InstrKind::LetI(variable("x"), text("bound"), Vec::new(), Vec::new()),
                3,
                span("let"),
            ),
            return_exp(variable("x"), 4),
        ],
    );
    let mut interpreter = interpreter(&[bind, no_leak]);

    assert_eq!(
        get::text(&interpreter.eval_func("bind", &[], &[]).unwrap()),
        Ok("bound")
    );
    let error = interpreter.eval_func("no_leak", &[], &[]).unwrap_err();
    assert!(error.message.contains("value `x` is undefined"));
}

#[test]
fn let_pattern_mismatch_continues_to_the_next_instruction() {
    let tuple_pattern = il::Exp::new(
        il::ExpKind::TupleE(vec![variable("x")]),
        il::TypKind::TupleT(vec![make_type::text_type()]),
        span("tuple-pattern"),
    );
    let mismatch = function(
        "mismatch",
        vec![
            sl::Instr::new(
                sl::InstrKind::LetI(
                    tuple_pattern,
                    text("not-a-tuple"),
                    Vec::new(),
                    vec![return_exp(text("wrong"), 2)],
                ),
                1,
                span("let"),
            ),
            return_exp(text("fallback"), 3),
        ],
    );
    let mut interpreter = interpreter(&[mismatch]);

    assert_eq!(
        get::text(&interpreter.eval_func("mismatch", &[], &[]).unwrap()),
        Ok("fallback")
    );
}
