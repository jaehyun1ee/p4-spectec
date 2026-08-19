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

fn guarded_function(name: &str, typ: il::Typ, guard: sl::Guard) -> sl::Def {
    let parameter_exp = il::Exp::new(
        il::ExpKind::VarE(id("value")),
        typ.node.clone(),
        span("value"),
    );
    let parameter = Spanned::new(
        sl::ParamKind::ExpP(typ, parameter_exp.clone()),
        span("parameter"),
    );
    function(
        name,
        vec![parameter],
        vec![sl::Instr::new(
            sl::InstrKind::CaseI(
                parameter_exp,
                vec![(guard, vec![return_text(name, 2)])],
                false,
            ),
            1,
            span("case"),
        )],
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

#[test]
fn case_instruction_preserves_every_guard_kind() {
    let bool_type = make_type::bool_type();
    let list_type = make_type::list_type(bool_type.clone());
    let bool_list = il::Exp::new(
        il::ExpKind::ListE(vec![bool_exp(true)]),
        list_type.node.clone(),
        span("list"),
    );
    let spec = [
        guarded_function(
            "comparison",
            bool_type.clone(),
            sl::Guard::CmpG(il::CmpOp::EqOp, il::OpTyp::BoolT, bool_exp(true)),
        ),
        guarded_function(
            "subtype",
            bool_type.clone(),
            sl::Guard::SubG(bool_type.clone()),
        ),
        guarded_function(
            "pattern",
            list_type.clone(),
            sl::Guard::MatchG(il::Pattern::ListP(il::ListPattern::Fixed(1))),
        ),
        guarded_function("membership", bool_type, sl::Guard::MemG(bool_list)),
    ];
    let mut interpreter = interpreter(&spec);
    let bool_value = make::bool(true, span("bool-value"));
    let list_value = make::list(
        &list_type,
        vec![make::bool(true, span("list-value"))],
        span("list-value"),
    );

    for name in ["comparison", "subtype", "membership"] {
        assert_eq!(
            get::text(
                &interpreter
                    .eval_func(name, &[], std::slice::from_ref(&bool_value))
                    .unwrap()
            ),
            Ok(name)
        );
    }
    assert_eq!(
        get::text(
            &interpreter
                .eval_func("pattern", &[], &[list_value])
                .unwrap()
        ),
        Ok("pattern")
    );
}
