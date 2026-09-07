use p4spec_rust::{
    interp::al::error::{ContextErrorKind, EntityKind, Error, ErrorKind},
    lang::common::source::{Position, Span},
};

fn trace(message: &str, children: Vec<Error>) -> Error {
    Error {
        span: Span::default(),
        kind: Box::new(ErrorKind::Context(ContextErrorKind::Undefined {
            kind: EntityKind::Value,
            name: message.into(),
        })),
        children,
    }
}

#[test]
fn test_failure_rendering_retains_branch_order_and_locations() {
    let span = Span::new(Position::new("spec", 3, 4), Position::new("spec", 3, 5));
    let mut first = trace("first mismatch", vec![]);
    first.span = span.clone();
    let error = Error::execution(vec![trace(
        "call failed",
        vec![first, trace("second mismatch", vec![])],
    )]);
    assert_eq!(
        error.to_string(),
        format!(
            "value `call failed` is undefined\n├── {span}\n    1. value `first mismatch` is undefined\n└── 2. value `second mismatch` is undefined\n"
        )
    );
}

#[test]
fn test_failure_rendering_bounds_deep_traces_and_keeps_root_and_leaf() {
    let mut nested = trace("leaf", vec![]);
    for index in (0..15).rev() {
        nested = trace(&format!("frame {index}"), vec![nested]);
    }
    let message = Error::execution(vec![nested]).to_string();
    assert!(message.starts_with(
        "value `frame 0` is undefined\n│ ··· omitting 5 traces ···\nvalue `frame 6` is undefined\n"
    ));
    assert!(message.contains("value `leaf` is undefined\n"));
    assert!(!message.contains("value `frame 1` is undefined\n"));
}
