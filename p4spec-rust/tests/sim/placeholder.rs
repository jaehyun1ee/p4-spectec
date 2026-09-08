use p4spec_rust::yojson::ExternalData;
use p4spec_rust::{
    lang::common::source::Span,
    lang::data::{
        typ::TypKind,
        value::{Value, ValueKind, make},
    },
    sim::placeholder::Placeholder,
};

use super::{has_extern_failure, parse_program, repo, runner, runner_from_spec};

fn contains_null_object_state(value: &Value) -> bool {
    let is_null_object_state = matches!(
        (&value.note, &value.node),
        (TypKind::Var(id, targs), ValueKind::Extern(ExternalData::Null))
            if id.node == "objectState" && targs.is_empty()
    );
    is_null_object_state
        || match &value.node {
            ValueKind::Struct(fields) => fields
                .iter()
                .any(|(_, value)| contains_null_object_state(value)),
            ValueKind::Case(value_case) => value_case
                .args()
                .into_iter()
                .any(|value| contains_null_object_state(value)),
            ValueKind::Tuple(values) | ValueKind::List(values) => {
                values.iter().any(|value| contains_null_object_state(value))
            }
            ValueKind::Opt(Some(value)) => contains_null_object_state(value),
            _ => false,
        }
}

#[test]
fn test_program_inst_initializes_placeholder_object() {
    let mut runner = runner(Placeholder);
    let program = parse_program(
        &repo()
            .join("p4c/testdata/p4_16_samples")
            .join("action_profile-bmv2.p4"),
    );

    let values = runner.eval_program("Program_inst", program).unwrap();

    assert!(values.iter().any(|value| contains_null_object_state(value)));
}

#[test]
fn test_unsupported_extern_fails() {
    let spec = repo().join("p4spec-rust/tests/fixtures/sim/unsupported-extern.watsup");
    let mut runner = runner_from_spec(&spec, Placeholder);

    let error = runner
        .eval_rel("Unsupported", &[make::bool(true, Span::default())])
        .unwrap_err();

    assert!(has_extern_failure(
        &error,
        "unimplemented extern relation: Unsupported"
    ));

    let value_ctx = make::bool(true, Span::default());
    let value_name = make::text("static_assert".to_owned(), Span::default());
    let typ_name = p4spec_rust::lang::data::typ::make::var(
        p4spec_rust::phrase!(node: "nameIR".to_owned(), span: Span::default()),
        Vec::new(),
    );
    let typ_names = p4spec_rust::lang::data::typ::make::list(typ_name);
    let value_names = make::list(
        &typ_names,
        ["message", "check"]
            .into_iter()
            .map(|name| make::text(name.to_owned(), Span::default()))
            .collect(),
        Span::default(),
    );
    let error = runner
        .eval_rel(
            "ExternFunctionCall_eval_lctk",
            &[value_ctx, value_name, value_names],
        )
        .unwrap_err();

    assert!(has_extern_failure(
        &error,
        "unsupported local compile-time known extern function call: static_assert(message, check)"
    ));
}
