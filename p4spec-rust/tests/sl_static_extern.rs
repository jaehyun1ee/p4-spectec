use p4spec_rust::{
    domain::{
        atom::Atom,
        mixfix::Mixfix,
        source::{Region, Spanned},
    },
    interface::{NullInterface, PlaceholderExtern},
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

fn variable_parameter(name: &str) -> sl::Param {
    Spanned::new(
        sl::ParamKind::ExpP(
            make_type::text_type(),
            il::Exp::new(il::ExpKind::VarE(id(name)), il::TypKind::TextT, span(name)),
        ),
        span("parameter"),
    )
}

fn p4_bool_exp(value: bool) -> il::Exp {
    let bool_type = make_type::var_type(id("boolValue"), Vec::new());
    let value = il::Exp::new(il::ExpKind::BoolE(value), il::TypKind::BoolT, span("bool"));
    let notation = Mixfix::Seq(vec![
        Mixfix::Atom(Spanned::new(Atom::Tag("B".to_owned()), span("tag"))),
        Mixfix::Arg(value),
    ]);
    il::Exp::new(
        il::ExpKind::CaseE(Box::new(notation)),
        bool_type.node,
        span("p4-bool"),
    )
}

fn static_extern_spec() -> Vec<sl::Def> {
    let relation = Spanned::new(
        sl::DefKind::ExternRelD((
            id("ExternFunctionCall_eval_lctk"),
            (
                Spanned::new(Mixfix::Seq(Vec::new()), span("signature")),
                Vec::new(),
            ),
            Vec::new(),
            Vec::new(),
        )),
        span("extern-relation"),
    );
    let find_local = Spanned::new(
        sl::DefKind::FuncDecD((
            id("find_var_value_t"),
            Vec::new(),
            vec![
                variable_parameter("prefixed_name"),
                variable_parameter("cursor"),
                variable_parameter("context"),
            ],
            make_type::var_type(id("boolValue"), Vec::new()),
            vec![sl::Instr::new(
                sl::InstrKind::ReturnI(p4_bool_exp(true)),
                1,
                span("return"),
            )],
            None,
            Vec::new(),
        )),
        span("find-local"),
    );
    vec![relation, find_local]
}

#[test]
fn placeholder_static_assert_calls_back_into_the_sl_spec() {
    let mut interpreter = Interpreter::new(
        &static_extern_spec(),
        Options {
            cache: true,
            deterministic: false,
            guard: false,
        },
        NullInterface,
        PlaceholderExtern::new(),
    )
    .unwrap();
    let context = make::text("typing-context".to_owned(), span("context"));
    let names = make::list(
        &make_type::list_type(make_type::text_type()),
        ["check"]
            .into_iter()
            .map(|name| make::text(name.to_owned(), span("name")))
            .collect(),
        span("names"),
    );

    let values = interpreter
        .eval_rel(
            "ExternFunctionCall_eval_lctk",
            &[
                context,
                make::text("static_assert".to_owned(), span("function")),
                names,
            ],
        )
        .unwrap();

    assert_eq!(values.len(), 1);
    let value_case = get::case(&values[0]).unwrap();
    assert_eq!(get::bool(value_case.args()[0]), Ok(true));
}
