use p4spec_rust::{
    domain::{
        atom::Atom,
        external_data::ExternalData,
        mixfix::{Mixfix, Mixop},
        source::{Region, Spanned},
    },
    interface::{Extern, ExternError, NullExtern, PlaceholderExtern},
    runtime::{
        r#type::typ::make as make_type,
        value::{ValueRef, get, make},
    },
};

fn span(file: &str) -> Region {
    Region::for_file(file)
}

fn p4_bool(value: bool) -> ValueRef {
    let mixop = Mixfix::Seq(vec![
        Mixfix::Atom(Spanned::new(Atom::Tag("B".to_owned()), Region::none())),
        Mixfix::Arg(()),
    ]);
    make::case(
        &make_type::var_type(
            Spanned::new("boolValue".to_owned(), Region::none()),
            Vec::new(),
        ),
        Mixop::fill(&mixop, [make::bool(value, span("bool"))]).unwrap(),
        span("bool-value"),
    )
}

fn p4_string(value: &str) -> ValueRef {
    let quote = Atom::Operator("\"".to_owned());
    let mixop = Mixfix::Seq(vec![
        Mixfix::Atom(Spanned::new(quote.clone(), Region::none())),
        Mixfix::Arg(()),
        Mixfix::Atom(Spanned::new(quote, Region::none())),
    ]);
    make::case(
        &make_type::var_type(
            Spanned::new("stringLiteral".to_owned(), Region::none()),
            Vec::new(),
        ),
        Mixop::fill(&mixop, [make::text(value.to_owned(), span("string"))]).unwrap(),
        span("string-literal"),
    )
}

fn names(values: &[&str]) -> ValueRef {
    make::list(
        &make_type::list_type(make_type::text_type()),
        values
            .iter()
            .map(|value| make::text((*value).to_owned(), span("name")))
            .collect(),
        span("names"),
    )
}

#[test]
fn null_extern_fails_through_the_object_safe_trait() {
    let mut externs: Box<dyn Extern> = Box::new(NullExtern);
    let error = externs.eval_rel("missing", &[]).unwrap_err();
    assert_eq!(error.span, Region::none());
    assert!(error.message.contains("extern is not configured"));
    assert!(!externs.side_effected(externs.checkpoint(), externs.checkpoint()));
}

#[test]
fn placeholder_initializers_return_null_external_state_values() {
    let mut externs = PlaceholderExtern::new(|_, _| {
        Err(ExternError::new(Region::none(), "lookup should not run"))
    });
    let object = externs.eval_func("init_objectState", &[], &[]).unwrap();
    let arch = externs.eval_func("init_archState", &[], &[]).unwrap();
    assert_eq!(get::external(&object), Ok(&ExternalData::Null));
    assert_eq!(get::external(&arch), Ok(&ExternalData::Null));
    assert_eq!(externs.checkpoint(), 0);
}

#[test]
fn static_assert_returns_true_and_uses_the_optional_message_on_failure() {
    let context = make::text("context".to_owned(), span("context"));
    let check_true = p4_bool(true);
    let message = p4_string("compile-time failure");
    let context_for_lookup = context.clone();
    let check_true_for_lookup = check_true.clone();
    let message_for_lookup = message.clone();
    let mut externs = PlaceholderExtern::new(move |received: &ValueRef, name: &str| {
        assert!(std::rc::Rc::ptr_eq(received, &context_for_lookup));
        match name {
            "check" => Ok(check_true_for_lookup.clone()),
            "message" => Ok(message_for_lookup.clone()),
            _ => Err(ExternError::new(Region::none(), "unknown local")),
        }
    });
    let passed = externs
        .eval_rel(
            "ExternFunctionCall_eval_lctk",
            &[
                context,
                make::text("static_assert".to_owned(), span("function")),
                names(&["check", "message"]),
            ],
        )
        .unwrap();
    assert_eq!(passed.len(), 1);
    assert_eq!(passed[0], check_true);

    let check_false = p4_bool(false);
    let false_for_lookup = check_false.clone();
    let message_for_lookup = message.clone();
    let context = make::text("false-context".to_owned(), span("context"));
    let context_for_lookup = context.clone();
    let mut externs = PlaceholderExtern::new(move |received: &ValueRef, name: &str| {
        assert!(std::rc::Rc::ptr_eq(received, &context_for_lookup));
        Ok(match name {
            "check" => false_for_lookup.clone(),
            "message" => message_for_lookup.clone(),
            _ => unreachable!(),
        })
    });
    let error = externs
        .eval_rel(
            "ExternFunctionCall_eval_lctk",
            &[
                context,
                make::text("static_assert".to_owned(), span("function")),
                names(&["check", "message"]),
            ],
        )
        .unwrap_err();
    assert_eq!(error.span, Region::none());
    assert_eq!(error.message, "compile-time failure");
}

#[test]
fn placeholder_rejects_unsupported_compile_time_externs() {
    let mut externs = PlaceholderExtern::new(|_, _| {
        Err(ExternError::new(Region::none(), "lookup should not run"))
    });
    let error = externs
        .eval_rel(
            "ExternFunctionCall_eval_lctk",
            &[
                make::text("context".to_owned(), span("context")),
                make::text("other".to_owned(), span("function")),
                names(&[]),
            ],
        )
        .unwrap_err();
    assert!(
        error
            .message
            .contains("unsupported local compile-time known extern function call")
    );

    let error = externs.eval_rel("ExternMethodCall_eval", &[]).unwrap_err();
    assert!(error.message.contains("not implemented"));
}
