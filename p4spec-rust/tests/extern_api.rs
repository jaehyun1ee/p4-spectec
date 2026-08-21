use p4spec_rust::{
    domain::{
        atom::Atom,
        external_data::ExternalData,
        mixfix::{Mixfix, Mixop},
        source::{Region, Spanned},
    },
    interface::{Extern, ExternError, NullExtern, PlaceholderExtern, SpecCall},
    lang::il::ast::Typ,
    runtime::{
        r#type::typ::make as make_type,
        value::{ValueRef, get, make},
    },
};

struct Lookup {
    context: ValueRef,
    names: std::collections::VecDeque<String>,
    values: std::collections::VecDeque<ValueRef>,
    calls: usize,
}

impl Lookup {
    fn new(
        context: ValueRef,
        names: impl IntoIterator<Item = &'static str>,
        values: impl IntoIterator<Item = ValueRef>,
    ) -> Self {
        Self {
            context,
            names: names.into_iter().map(str::to_owned).collect(),
            values: values.into_iter().collect(),
            calls: 0,
        }
    }
}

impl SpecCall for Lookup {
    fn eval_func(
        &mut self,
        name: &str,
        type_args: &[Typ],
        values: &[ValueRef],
    ) -> Result<ValueRef, ExternError> {
        assert_eq!(name, "find_var_value_t");
        assert!(type_args.is_empty());
        assert_eq!(values.len(), 3);
        assert!(std::rc::Rc::ptr_eq(&values[2], &self.context));
        let prefixed_name = get::case(&values[0]).unwrap();
        assert_eq!(
            prefixed_name.split().0,
            Mixfix::Seq(vec![
                Mixfix::Atom(Spanned::new(Atom::Tag("BARE".to_owned()), Region::none())),
                Mixfix::Arg(()),
            ])
        );
        assert_eq!(prefixed_name.args().len(), 1);
        assert_eq!(
            get::text(prefixed_name.args()[0]),
            Ok(self.names.pop_front().unwrap().as_str())
        );
        let cursor = get::case(&values[1]).unwrap();
        assert_eq!(
            cursor.split().0,
            Mixfix::Atom(Spanned::new(
                Atom::Keyword("LOCAL".to_owned()),
                Region::none()
            ))
        );
        assert!(cursor.args().is_empty());
        self.calls += 1;
        self.values
            .pop_front()
            .ok_or_else(|| ExternError::new(Region::none(), "unexpected lookup"))
    }

    fn eval_rel(
        &mut self,
        _name: &str,
        _values: &[ValueRef],
    ) -> Result<Vec<ValueRef>, ExternError> {
        Err(ExternError::new(
            Region::none(),
            "unexpected relation lookup",
        ))
    }
}

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
    let context = make::text("unused".to_owned(), span("unused"));
    let mut lookup = Lookup::new(context, [], []);
    let error = externs.eval_rel(&mut lookup, "missing", &[]).unwrap_err();
    assert_eq!(error.span, Region::none());
    assert!(error.message.contains("extern is not configured"));
    assert!(!externs.side_effected(externs.checkpoint(), externs.checkpoint()));
}

#[test]
fn placeholder_initializers_return_null_external_state_values() {
    let mut externs = PlaceholderExtern::new();
    let context = make::text("unused".to_owned(), span("unused"));
    let mut lookup = Lookup::new(context, [], []);
    let object = externs
        .eval_func(&mut lookup, "init_objectState", &[], &[])
        .unwrap();
    let arch = externs
        .eval_func(&mut lookup, "init_archState", &[], &[])
        .unwrap();
    assert_eq!(get::external(&object), Ok(&ExternalData::Null));
    assert_eq!(get::external(&arch), Ok(&ExternalData::Null));
    assert_eq!(externs.checkpoint(), 0);
}

#[test]
fn static_assert_returns_true_and_uses_the_optional_message_on_failure() {
    let context = make::text("context".to_owned(), span("context"));
    let check_true = p4_bool(true);
    let message = p4_string("compile-time failure");
    let mut lookup = Lookup::new(
        context.clone(),
        ["check"],
        [check_true.clone(), message.clone()],
    );
    let mut externs = PlaceholderExtern::new();
    let passed = externs
        .eval_rel(
            &mut lookup,
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
    assert_eq!(lookup.calls, 1);

    let check_false = p4_bool(false);
    let context = make::text("false-context".to_owned(), span("context"));
    let mut lookup = Lookup::new(
        context.clone(),
        ["check", "message"],
        [check_false, message],
    );
    let mut externs = PlaceholderExtern::new();
    let error = externs
        .eval_rel(
            &mut lookup,
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
    assert_eq!(lookup.calls, 2);
}

#[test]
fn placeholder_rejects_unsupported_compile_time_externs() {
    let mut externs = PlaceholderExtern::new();
    let context = make::text("unused".to_owned(), span("unused"));
    let mut lookup = Lookup::new(context, [], []);
    let error = externs
        .eval_rel(
            &mut lookup,
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

    let error = externs
        .eval_rel(&mut lookup, "ExternMethodCall_eval", &[])
        .unwrap_err();
    assert!(error.message.contains("unimplemented extern relation"));

    let error = externs
        .eval_func(&mut lookup, "other", &[], &[])
        .unwrap_err();
    assert!(error.message.contains("unimplemented extern function"));
}
