use std::rc::Rc;

use num_bigint::BigInt;
use p4spec_rust::{
    domain::source::{Region, Spanned},
    interp::sl::{context::Context, expression},
    lang::{il::ast as il, xl::num},
    runtime::{
        dynamic::var::Variable,
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

fn variable(name: &str, ty: il::TypKind) -> il::Exp {
    il::Exp::new(il::ExpKind::VarE(id(name)), ty, span(name))
}

fn int_exp(value: i64) -> il::Exp {
    il::Exp::new(
        il::ExpKind::NumE(num::T::Int(BigInt::from(value))),
        il::TypKind::NumT(num::Typ::IntT),
        span("int"),
    )
}

fn iter_var(name: &str, ty: il::TypKind, iters: Vec<il::Iter>) -> il::Var {
    (id(name), Spanned::new(ty, span("var-type")), iters)
}

fn context() -> Context {
    let mut context = Context::from_spec(false, &[]).unwrap();
    context.enter_relation(id("R"), Vec::new());
    context
}

#[test]
fn simple_iterated_variable_returns_the_existing_container() {
    let mut context = context();
    let int_type = make_type::int_type();
    let list_type = make_type::list_type(int_type.clone());
    let values = make::list(
        &list_type,
        vec![make::int(1.into(), span("one"))],
        span("values"),
    );
    context
        .bind_value(Variable::new(id("x"), vec![il::Iter::List]), values.clone())
        .unwrap();
    let iterated = il::Exp::new(
        il::ExpKind::IterE(
            Box::new(variable("x", int_type.node.clone())),
            (
                il::Iter::List,
                vec![iter_var("x", int_type.node, Vec::new())],
            ),
        ),
        list_type.node,
        span("iterated"),
    );

    let result = expression::eval(&mut context, &iterated).unwrap();
    assert!(Rc::ptr_eq(&result, &values));
}

#[test]
fn list_iteration_evaluates_each_binding_and_restores_the_outer_scope() {
    let mut context = context();
    let int_type = make_type::int_type();
    let list_type = make_type::list_type(int_type.clone());
    let outer = make::int(99.into(), span("outer"));
    context
        .bind_value(Variable::new(id("x"), Vec::new()), outer.clone())
        .unwrap();
    context
        .bind_value(
            Variable::new(id("x"), vec![il::Iter::List]),
            make::list(
                &list_type,
                vec![
                    make::int(1.into(), span("one")),
                    make::int(2.into(), span("two")),
                ],
                span("xs"),
            ),
        )
        .unwrap();
    let body = il::Exp::new(
        il::ExpKind::BinE(
            il::BinOp::AddOp,
            il::OpTyp::IntT,
            Box::new(variable("x", int_type.node.clone())),
            Box::new(int_exp(10)),
        ),
        int_type.node.clone(),
        span("body"),
    );
    let iterated = il::Exp::new(
        il::ExpKind::IterE(
            Box::new(body),
            (
                il::Iter::List,
                vec![iter_var("x", int_type.node, Vec::new())],
            ),
        ),
        list_type.node,
        span("iterated"),
    );

    let result = expression::eval(&mut context, &iterated).unwrap();
    let numbers = get::list(&result)
        .unwrap()
        .iter()
        .map(|value| match get::num(value).unwrap() {
            num::T::Int(value) => value.clone(),
            num::T::Nat(_) => panic!("expected int"),
        })
        .collect::<Vec<_>>();
    assert_eq!(numbers, vec![11.into(), 12.into()]);
    assert!(Rc::ptr_eq(
        context
            .find_value(&Variable::new(id("x"), Vec::new()))
            .unwrap(),
        &outer,
    ));
}

#[test]
fn optional_iteration_skips_none_and_shares_the_some_child() {
    let mut context = context();
    let int_type = make_type::int_type();
    let input_option_type = make_type::opt_type(int_type.clone());
    let body_option_type = make_type::opt_type(int_type.clone());
    let result_option_type = make_type::opt_type(body_option_type.clone());
    context
        .bind_value(
            Variable::new(id("x"), vec![il::Iter::Opt]),
            make::opt(&input_option_type, None, span("none")),
        )
        .unwrap();
    let body = il::Exp::new(
        il::ExpKind::OptE(Some(Box::new(variable("x", int_type.node.clone())))),
        body_option_type.node.clone(),
        span("body"),
    );
    let iterated = il::Exp::new(
        il::ExpKind::IterE(
            Box::new(body),
            (
                il::Iter::Opt,
                vec![iter_var("x", int_type.node.clone(), Vec::new())],
            ),
        ),
        result_option_type.node,
        span("iterated"),
    );
    let result = expression::eval(&mut context, &iterated).unwrap();
    assert!(get::opt(&result).unwrap().is_none());

    let child = make::int(7.into(), span("child"));
    context
        .bind_value(
            Variable::new(id("x"), vec![il::Iter::Opt]),
            make::opt(&input_option_type, Some(child.clone()), span("some")),
        )
        .unwrap();
    let result = expression::eval(&mut context, &iterated).unwrap();
    let body_result = get::opt(&result).unwrap().unwrap();
    assert!(Rc::ptr_eq(get::opt(body_result).unwrap().unwrap(), &child));
    assert!(!context.is_value_bound(&Variable::new(id("x"), Vec::new())));
}

#[test]
fn failed_iteration_restores_temporary_bindings() {
    let mut context = context();
    let int_type = make_type::int_type();
    let list_type = make_type::list_type(int_type.clone());
    context
        .bind_value(
            Variable::new(id("x"), vec![il::Iter::List]),
            make::list(
                &list_type,
                vec![make::int(1.into(), span("one"))],
                span("xs"),
            ),
        )
        .unwrap();
    let failing_body = il::Exp::new(
        il::ExpKind::IdxE(
            Box::new(il::Exp::new(
                il::ExpKind::TextE("a".to_owned()),
                il::TypKind::TextT,
                span("text"),
            )),
            Box::new(int_exp(2)),
        ),
        il::TypKind::TextT,
        span("failing-body"),
    );
    let iterated = il::Exp::new(
        il::ExpKind::IterE(
            Box::new(failing_body),
            (
                il::Iter::List,
                vec![iter_var("x", int_type.node, Vec::new())],
            ),
        ),
        list_type.node,
        span("iterated"),
    );

    assert!(expression::eval(&mut context, &iterated).is_err());
    assert!(!context.is_value_bound(&Variable::new(id("x"), Vec::new())));
}
