use p4spec_rust::{
    domain::source::{Region, Spanned},
    interface::{BuiltinInterface, NullExtern},
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

fn bool_exp(value: bool) -> il::Exp {
    il::Exp::new(il::ExpKind::BoolE(value), il::TypKind::BoolT, span("bool"))
}

fn text_exp(value: &str) -> il::Exp {
    il::Exp::new(
        il::ExpKind::TextE(value.to_owned()),
        il::TypKind::TextT,
        span("text"),
    )
}

fn return_text(value: &str, iid: i64) -> sl::Instr {
    sl::Instr::new(sl::InstrKind::ReturnI(text_exp(value)), iid, span("return"))
}

fn function(name: &str, params: Vec<sl::Param>, block: sl::Block) -> sl::Def {
    Spanned::new(
        sl::DefKind::FuncDecD((
            id(name),
            Vec::new(),
            params,
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
fn if_instruction_runs_true_block_and_false_condition_continues() {
    let choose_true = function(
        "choose_true",
        Vec::new(),
        vec![
            sl::Instr::new(
                sl::InstrKind::IfI(
                    bool_exp(true),
                    Vec::new(),
                    vec![return_text("then", 2)],
                    false,
                ),
                1,
                span("if"),
            ),
            return_text("fallback", 3),
        ],
    );
    let choose_false = function(
        "choose_false",
        Vec::new(),
        vec![
            sl::Instr::new(
                sl::InstrKind::IfI(
                    bool_exp(false),
                    Vec::new(),
                    vec![return_text("then", 5)],
                    false,
                ),
                4,
                span("if"),
            ),
            return_text("fallback", 6),
        ],
    );
    let mut interpreter = interpreter(&[choose_true, choose_false]);

    assert_eq!(
        get::text(&interpreter.eval_func("choose_true", &[], &[]).unwrap()),
        Ok("then")
    );
    assert_eq!(
        get::text(&interpreter.eval_func("choose_false", &[], &[]).unwrap()),
        Ok("fallback")
    );
}

#[test]
fn case_instruction_evaluates_guards_in_order_and_selects_one_block() {
    let parameter_exp = il::Exp::new(
        il::ExpKind::VarE(id("condition")),
        il::TypKind::BoolT,
        span("condition"),
    );
    let parameter = Spanned::new(
        sl::ParamKind::ExpP(make_type::bool_type(), parameter_exp.clone()),
        span("parameter"),
    );
    let cases = vec![
        (sl::Guard::BoolG(true), vec![return_text("true", 2)]),
        (sl::Guard::BoolG(false), vec![return_text("false", 3)]),
    ];
    let classify = function(
        "classify",
        vec![parameter],
        vec![sl::Instr::new(
            sl::InstrKind::CaseI(parameter_exp, cases, false),
            1,
            span("case"),
        )],
    );
    let mut interpreter = interpreter(&[classify]);

    assert_eq!(
        get::text(
            &interpreter
                .eval_func("classify", &[], &[make::bool(true, span("true"))])
                .unwrap()
        ),
        Ok("true")
    );
    assert_eq!(
        get::text(
            &interpreter
                .eval_func("classify", &[], &[make::bool(false, span("false"))])
                .unwrap()
        ),
        Ok("false")
    );
}
