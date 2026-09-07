use p4spec_rust::{
    interp::al::{backtrack::FailTrace, error::ErrorKind},
    lang::common::source::{Position, Span},
};

fn trace(message: &str, children: Vec<FailTrace>) -> FailTrace {
    FailTrace {
        span: Span::default(),
        message: message.into(),
        children,
    }
}

#[test]
fn test_failure_rendering_retains_branch_order_and_locations() {
    let span = Span::new(Position::new("spec", 3, 4), Position::new("spec", 3, 5));
    let mut first = trace("first mismatch", vec![]);
    first.span = span.clone();
    let error = ErrorKind::Execution(vec![trace(
        "call failed",
        vec![first, trace("second mismatch", vec![])],
    )]);
    assert_eq!(
        error.to_string(),
        format!("call failed\n├── {span}\n    1. first mismatch\n└── 2. second mismatch\n")
    );
}

#[test]
fn test_failure_rendering_bounds_deep_traces_and_keeps_root_and_leaf() {
    let mut nested = trace("leaf", vec![]);
    for index in (0..15).rev() {
        nested = trace(&format!("frame {index}"), vec![nested]);
    }
    let message = ErrorKind::Execution(vec![nested]).to_string();
    assert!(message.starts_with("frame 0\n│ ··· omitting 5 traces ···\nframe 6\n"));
    assert!(message.contains("leaf\n"));
    assert!(!message.contains("frame 1\n"));
}
