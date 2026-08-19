use p4spec_rust::{
    domain::source::{Region, Spanned},
    interface::{NullExtern, NullInterface},
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

fn iter_var(name: &str, ty: il::TypKind) -> il::Var {
    (id(name), Spanned::new(ty, span("var-type")), Vec::new())
}

fn variable(name: &str) -> il::Exp {
    il::Exp::new(il::ExpKind::VarE(id(name)), il::TypKind::BoolT, span(name))
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

fn all_true() -> sl::Def {
    let bool_type = make_type::bool_type();
    let list_type = make_type::list_type(bool_type.clone());
    let variable = variable("item");
    let var = iter_var("item", bool_type.node.clone());
    let parameter_pattern = il::Exp::new(
        il::ExpKind::IterE(
            Box::new(variable.clone()),
            (il::Iter::List, vec![var.clone()]),
        ),
        list_type.node.clone(),
        span("parameter-pattern"),
    );
    let parameter = Spanned::new(
        sl::ParamKind::ExpP(list_type, parameter_pattern),
        span("parameter"),
    );
    let if_instr = sl::Instr::new(
        sl::InstrKind::IfI(
            variable,
            vec![(il::Iter::List, vec![var])],
            vec![return_text("all", 2)],
            false,
        ),
        1,
        span("if"),
    );
    Spanned::new(
        sl::DefKind::FuncDecD((
            id("all_true"),
            Vec::new(),
            vec![parameter],
            make_type::text_type(),
            vec![if_instr, return_text("not-all", 3)],
            None,
            Vec::new(),
        )),
        span("function"),
    )
}

#[test]
fn list_iterated_if_requires_every_element_and_accepts_empty_list() {
    let mut interpreter = Interpreter::new(
        &[all_true()],
        Options {
            cache: false,
            deterministic: false,
            guard: false,
        },
        NullInterface,
        NullExtern,
    )
    .unwrap();
    let list_type = make_type::list_type(make_type::bool_type());
    let list = |values: &[bool]| {
        make::list(
            &list_type,
            values
                .iter()
                .map(|value| make::bool(*value, span("item")))
                .collect(),
            span("list"),
        )
    };

    for (values, expected) in [
        (vec![true, true], "all"),
        (vec![true, false], "not-all"),
        (Vec::new(), "all"),
    ] {
        let input = list(&values);
        assert_eq!(
            get::text(
                &interpreter
                    .eval_func("all_true", &[], std::slice::from_ref(&input))
                    .unwrap()
            ),
            Ok(expected)
        );
    }
}
