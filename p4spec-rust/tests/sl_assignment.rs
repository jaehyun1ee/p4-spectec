use p4spec_rust::{
    domain::source::{Region, Spanned},
    interp::sl::{assignment, context::Context},
    lang::il::ast as il,
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

fn variable(name: &str, typ: il::TypKind) -> il::Exp {
    il::Exp::new(il::ExpKind::VarE(id(name)), typ, span(name))
}

fn context() -> Context {
    let mut context = Context::from_spec(false, &[]).unwrap();
    context.enter_relation(id("R"), Vec::new());
    context
}

#[test]
fn tuple_assignment_binds_each_variable() {
    let mut context = context();
    let pattern = il::Exp::new(
        il::ExpKind::TupleE(vec![
            variable("x", il::TypKind::BoolT),
            variable("y", il::TypKind::TextT),
        ]),
        il::TypKind::TupleT(Vec::new()),
        span("tuple"),
    );
    let tuple_type = make_type::tuple_type(vec![make_type::bool_type(), make_type::text_type()]);
    let value_x = make::bool(true, span("x-value"));
    let value_y = make::text("hello".to_owned(), span("y-value"));
    let value = make::tuple(
        &tuple_type,
        vec![value_x.clone(), value_y.clone()],
        span("value"),
    );
    assignment::assign(&mut context, &pattern, value).unwrap();
    assert_eq!(
        context
            .find_value(&Variable::new(id("x"), Vec::new()))
            .unwrap(),
        &value_x,
    );
    assert_eq!(
        context
            .find_value(&Variable::new(id("y"), Vec::new()))
            .unwrap(),
        &value_y,
    );
}

#[test]
fn failed_nested_assignment_restores_every_earlier_binding() {
    let mut context = context();
    let existing = make::bool(false, span("existing"));
    context
        .bind_value(Variable::new(id("x"), Vec::new()), existing.clone())
        .unwrap();
    let list_pattern = il::Exp::new(
        il::ExpKind::ListE(vec![variable("z", il::TypKind::BoolT)]),
        il::TypKind::IterT(Box::new(make_type::bool_type()), il::Iter::List),
        span("list-pattern"),
    );
    let pattern = il::Exp::new(
        il::ExpKind::TupleE(vec![variable("x", il::TypKind::BoolT), list_pattern]),
        il::TypKind::TupleT(Vec::new()),
        span("tuple"),
    );
    let value = make::tuple(
        &make_type::tuple_type(vec![make_type::bool_type(), make_type::bool_type()]),
        vec![
            make::bool(true, span("replacement")),
            make::bool(true, span("wrong")),
        ],
        span("value"),
    );
    let error = assignment::assign(&mut context, &pattern, value).unwrap_err();
    assert!(error.message.contains("match failed"));
    assert_eq!(
        context
            .find_value(&Variable::new(id("x"), Vec::new()))
            .unwrap(),
        &existing,
    );
    assert!(!context.is_value_bound(&Variable::new(id("z"), Vec::new())));
}

#[test]
fn option_list_and_cons_assignment_preserve_shared_children() {
    let mut context = context();
    let option_pattern = il::Exp::new(
        il::ExpKind::OptE(Some(Box::new(variable("option", il::TypKind::BoolT)))),
        il::TypKind::IterT(Box::new(make_type::bool_type()), il::Iter::Opt),
        span("option-pattern"),
    );
    let inner = make::bool(true, span("inner"));
    assignment::assign(
        &mut context,
        &option_pattern,
        make::opt(
            &make_type::opt_type(make_type::bool_type()),
            Some(inner.clone()),
            span("option"),
        ),
    )
    .unwrap();
    assert_eq!(
        context
            .find_value(&Variable::new(id("option"), Vec::new()))
            .unwrap(),
        &inner
    );

    let cons_pattern = il::Exp::new(
        il::ExpKind::ConsE(
            Box::new(variable("head", il::TypKind::BoolT)),
            Box::new(variable(
                "tail",
                il::TypKind::IterT(Box::new(make_type::bool_type()), il::Iter::List),
            )),
        ),
        il::TypKind::IterT(Box::new(make_type::bool_type()), il::Iter::List),
        span("cons"),
    );
    let head = make::bool(false, span("head"));
    let next = make::bool(true, span("next"));
    assignment::assign(
        &mut context,
        &cons_pattern,
        make::list(
            &make_type::list_type(make_type::bool_type()),
            vec![head.clone(), next.clone()],
            span("list"),
        ),
    )
    .unwrap();
    assert_eq!(
        context
            .find_value(&Variable::new(id("head"), Vec::new()))
            .unwrap(),
        &head
    );
    let tail = context
        .find_value(&Variable::new(id("tail"), Vec::new()))
        .unwrap();
    assert_eq!(get::list(tail).unwrap(), std::slice::from_ref(&next));
}

#[test]
fn fixed_list_arity_mismatch_is_an_error_without_bindings() {
    let mut context = context();
    let pattern = il::Exp::new(
        il::ExpKind::ListE(vec![variable("only", il::TypKind::BoolT)]),
        il::TypKind::IterT(Box::new(make_type::bool_type()), il::Iter::List),
        span("pattern"),
    );
    let value = make::list(
        &make_type::list_type(make_type::bool_type()),
        Vec::new(),
        span("empty"),
    );
    assert!(assignment::assign(&mut context, &pattern, value).is_err());
    assert!(!context.is_value_bound(&Variable::new(id("only"), Vec::new())));
}
