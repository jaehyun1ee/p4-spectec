use p4spec_rust::interp::shared::error::{ContextErrorKind, RuntimeErrorKind};
use p4spec_rust::{
    interp::shared::{
        backtrack::Backtrack,
        error::{EntityKind, Error, ErrorKind},
    },
    lang::common::source::{Position, Span},
};

fn undefined(name: impl Into<String>) -> ErrorKind {
    ErrorKind::Context(ContextErrorKind::Undefined { kind: EntityKind::Value, name: name.into() })
}

fn span(line: usize) -> Span {
    Span::new(Position::new("choice.watsup", line, 0), Position::new("choice.watsup", line, 1))
}

#[test]
fn test_nesting_keeps_failure_class_locations_and_children() {
    for fatal in [false, true] {
        let result: Backtrack<()> = if fatal {
            Backtrack::err(span(2), undefined("leaf"))
        } else {
            Backtrack::unmatch(span(2), undefined("leaf"))
        };
        let result = result.nest(span(1), || undefined("call"));
        let traces = vec![Error {
            span: span(1),
            kind: Box::new(undefined("call")),
            children: vec![Error {
                span: span(2),
                kind: Box::new(undefined("leaf")),
                children: vec![],
            }],
        }];
        assert_eq!(result, if fatal { Backtrack::Err(traces) } else { Backtrack::Unmatch(traces) });
    }
    assert_eq!(
        Backtrack::<_>::Ok(7).nest(span(1), || panic!("success formatted a failure")),
        Backtrack::Ok(7)
    );
    assert_eq!(
        Backtrack::<()>::Unmatch(vec![]).nest(span(1), || undefined("call")),
        Backtrack::Unmatch(vec![Error {
            span: span(1),
            kind: Box::new(undefined("call")),
            children: vec![]
        }])
    );
}

#[test]
fn test_check_classifies_failure_as_fatal() {
    assert_eq!(
        Backtrack::check(false, span(1), undefined("failed")),
        Backtrack::err(span(1), undefined("failed"))
    );
    assert_eq!(Backtrack::check(true, span(1), undefined("failed")), Backtrack::Ok(()));
}

#[test]
fn test_from_result_preserves_typed_nested_errors_and_existing_locations() {
    let error = Error {
        kind: Box::new(undefined("outer")),
        span: span(1),
        children: vec![Error::new(
            ErrorKind::Context(ContextErrorKind::OptionalityMismatch),
            span(2),
        )],
    };
    let result = Backtrack::<()>::from_result(Err(error.clone()), &span(3));
    assert_eq!(result, Backtrack::Err(vec![error]));
}

#[test]
fn test_from_result_locates_unlocated_runtime_errors() {
    let error = p4spec_rust::lang::xl::num::NumericError::NegativeNatural((-1).into());
    let result = Backtrack::<()>::from_result(Err(error.clone()), &span(3));
    assert_eq!(
        result,
        Backtrack::Err(vec![Error::new(
            ErrorKind::Runtime(RuntimeErrorKind::Numeric(error)),
            span(3)
        )])
    );
}
