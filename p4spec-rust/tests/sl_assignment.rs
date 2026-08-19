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

fn iterated_tuple(iter: il::Iter) -> il::Exp {
    let bool_type = make_type::bool_type();
    let text_type = make_type::text_type();
    let tuple = il::Exp::new(
        il::ExpKind::TupleE(vec![
            variable("x", il::TypKind::BoolT),
            variable("y", il::TypKind::TextT),
        ]),
        make_type::tuple_kind(vec![bool_type.clone(), text_type.clone()]),
        span("tuple"),
    );
    il::Exp::new(
        il::ExpKind::IterE(
            Box::new(tuple),
            (
                iter,
                vec![
                    (id("x"), bool_type, Vec::new()),
                    (id("y"), text_type, Vec::new()),
                ],
            ),
        ),
        il::TypKind::IterT(
            Box::new(make_type::tuple_type(vec![
                make_type::bool_type(),
                make_type::text_type(),
            ])),
            iter,
        ),
        span("iteration"),
    )
}

fn tuple_value(value_bool: bool, value_text: &str) -> p4spec_rust::runtime::value::ValueRef {
    make::tuple(
        &make_type::tuple_type(vec![make_type::bool_type(), make_type::text_type()]),
        vec![
            make::bool(value_bool, span("bool")),
            make::text(value_text.to_owned(), span("text")),
        ],
        span("tuple-value"),
    )
}

#[test]
fn complex_list_iteration_transposes_assigned_variables() {
    let mut context = context();
    let first = tuple_value(true, "first");
    let second = tuple_value(false, "second");
    let value = make::list(
        &make_type::list_type(make_type::tuple_type(vec![
            make_type::bool_type(),
            make_type::text_type(),
        ])),
        vec![first, second],
        span("list"),
    );

    assignment::assign(&mut context, &iterated_tuple(il::Iter::List), value).unwrap();

    let values_x = context
        .find_value(&Variable::new(id("x"), vec![il::Iter::List]))
        .unwrap();
    let values_x = get::list(values_x).unwrap();
    assert_eq!(get::bool(&values_x[0]), Ok(true));
    assert_eq!(get::bool(&values_x[1]), Ok(false));
    let values_y = context
        .find_value(&Variable::new(id("y"), vec![il::Iter::List]))
        .unwrap();
    let values_y = get::list(values_y).unwrap();
    assert_eq!(get::text(&values_y[0]), Ok("first"));
    assert_eq!(get::text(&values_y[1]), Ok("second"));
    assert!(!context.is_value_bound(&Variable::new(id("x"), Vec::new())));
    assert!(!context.is_value_bound(&Variable::new(id("y"), Vec::new())));
}

#[test]
fn complex_optional_iteration_wraps_some_and_none_per_variable() {
    let mut context = context();
    let tuple_type = make_type::tuple_type(vec![make_type::bool_type(), make_type::text_type()]);
    let value = make::opt(
        &make_type::opt_type(tuple_type.clone()),
        Some(tuple_value(true, "inside")),
        span("some"),
    );
    assignment::assign(&mut context, &iterated_tuple(il::Iter::Opt), value).unwrap();
    assert_eq!(
        get::bool(
            get::opt(
                context
                    .find_value(&Variable::new(id("x"), vec![il::Iter::Opt]))
                    .unwrap()
            )
            .unwrap()
            .unwrap()
        ),
        Ok(true)
    );
    assert_eq!(
        get::text(
            get::opt(
                context
                    .find_value(&Variable::new(id("y"), vec![il::Iter::Opt]))
                    .unwrap()
            )
            .unwrap()
            .unwrap()
        ),
        Ok("inside")
    );

    let value = make::opt(&make_type::opt_type(tuple_type), None, span("none"));
    assignment::assign(&mut context, &iterated_tuple(il::Iter::Opt), value).unwrap();
    assert!(
        get::opt(
            context
                .find_value(&Variable::new(id("x"), vec![il::Iter::Opt]))
                .unwrap()
        )
        .unwrap()
        .is_none()
    );
    assert!(
        get::opt(
            context
                .find_value(&Variable::new(id("y"), vec![il::Iter::Opt]))
                .unwrap()
        )
        .unwrap()
        .is_none()
    );
}
