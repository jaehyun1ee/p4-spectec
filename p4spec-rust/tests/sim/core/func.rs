use std::rc::Rc;

use p4spec_rust::{
    frontend::parse::parse_mixop,
    lang::{
        common::source::Span,
        data::{
            typ::make as make_typ,
            value::{Value, make},
        },
    },
    sim::placeholder::Placeholder,
};

use super::super::{has_extern_failure, parse_program, repo, runner};

fn case(shape: &str, args: Vec<Rc<Value>>, typ: &str) -> Rc<Value> {
    let mixop = parse_mixop(shape).unwrap();
    let value_case = p4spec_rust::lang::common::notation::mixop::Mixop::fill(&mixop, args).unwrap();
    let typ = make_typ::var(
        p4spec_rust::phrase!(node: typ.to_owned(), span: Span::default()),
        Vec::new(),
    );
    make::case_(&typ, value_case, Span::default())
}

fn run_static_assert(names_param: &[&str]) -> (Rc<Value>, Rc<Value>) {
    let spec = repo().join("p4spec-rust/tests/fixtures/sim/unsupported-extern.watsup");
    let mut runner = super::super::runner_from_spec(&spec, Placeholder);
    let value_check = case("_B bool", vec![make::bool(true, Span::default())], "value");
    let value_message = case(
        "'\"' text '\"'",
        vec![make::text("unused".to_owned(), Span::default())],
        "value",
    );
    let value_ctx = case(
        "CTX value value",
        vec![Rc::clone(&value_check), value_message],
        "typingContext",
    );
    let typ_name = make_typ::var(
        p4spec_rust::phrase!(node: "nameIR".to_owned(), span: Span::default()),
        Vec::new(),
    );
    let typ_names = make_typ::list(typ_name);
    let values_name = names_param
        .iter()
        .map(|name| make::text((*name).to_owned(), Span::default()))
        .collect();
    let value_names = make::list(&typ_names, values_name, Span::default());
    let value_name = make::text("static_assert".to_owned(), Span::default());

    let values = runner
        .eval_rel(
            "ExternFunctionCall_eval_lctk",
            &[value_ctx, value_name, value_names],
        )
        .unwrap();

    (value_check, Rc::clone(&values[0]))
}

#[test]
fn test_static_assert_returns_true() {
    for names_param in [&["check"][..], &["check", "message"][..]] {
        let (value_check, value_result) = run_static_assert(names_param);
        assert!(Rc::ptr_eq(&value_check, &value_result));
    }

    let mut runner = runner(Placeholder);
    let program =
        parse_program(&repo().join("p4spec-rust/tests/fixtures/sim/static-assert-true.p4"));

    runner.eval_program("Program_ok", program).unwrap();
}

#[test]
fn test_static_assert_false_default_message() {
    let mut runner = runner(Placeholder);
    let program = parse_program(
        &repo().join("p4spec-rust/tests/fixtures/sim/static-assert-false-default.p4"),
    );

    let error = runner.eval_program("Program_ok", program).unwrap_err();

    assert!(has_extern_failure(&error, "static_assert failed"));
}

#[test]
fn test_static_assert_false_custom_message() {
    let mut runner = runner(Placeholder);
    let program =
        parse_program(&repo().join("p4spec-rust/tests/fixtures/sim/static-assert-false-custom.p4"));

    let error = runner.eval_program("Program_ok", program).unwrap_err();

    assert!(has_extern_failure(&error, "custom assertion failure"));
}
