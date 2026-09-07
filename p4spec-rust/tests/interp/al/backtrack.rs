use p4spec_rust::interp::al::error::{ContextErrorKind, RuntimeErrorKind};
use p4spec_rust::{
    interp::al::{
        backtrack::{Backtrack, choose_deterministic, choose_sequential},
        error::{EntityKind, Error, ErrorKind},
    },
    lang::common::source::{Position, Span},
};

fn undefined(name: impl Into<String>) -> ErrorKind {
    ErrorKind::Context(ContextErrorKind::Undefined {
        kind: EntityKind::Value,
        name: name.into(),
    })
}

fn span(line: i64) -> Span {
    Span::new(
        Position::new("choice.watsup", line, 0),
        Position::new("choice.watsup", line, 1),
    )
}

#[test]
fn test_sequential_choice_stops_at_first_success_or_fatal_error() {
    for fatal in [false, true] {
        let mut visited = Vec::new();
        let result = choose_sequential([0, 1, 2], |index| {
            visited.push(*index);
            match index {
                0 => Backtrack::unmatch(span(1), undefined("first")),
                1 if fatal => Backtrack::err(span(2), undefined("fatal")),
                _ => Backtrack::Ok(42),
            }
        });
        assert_eq!(visited, [0, 1]);
        let expected = if fatal {
            Backtrack::err(span(2), undefined("fatal"))
        } else {
            Backtrack::Ok(42)
        };
        assert_eq!(result, expected);
    }
}

#[test]
fn test_all_unmatched_candidates_keep_trace_order() {
    let result: Backtrack<()> = choose_sequential([1, 2], |line| {
        Backtrack::unmatch(span(*line), undefined(format!("candidate {line}")))
    });
    assert_eq!(
        result,
        Backtrack::Unmatch(vec![
            Error {
                span: span(1),
                kind: Box::new(undefined("candidate 1")),
                children: vec![]
            },
            Error {
                span: span(2),
                kind: Box::new(undefined("candidate 2")),
                children: vec![]
            },
        ])
    );
    let empty: Backtrack<()> = choose_sequential([], |_: &()| panic!("empty choice evaluated"));
    assert_eq!(empty, Backtrack::Unmatch(vec![]));
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
        assert_eq!(
            result,
            if fatal {
                Backtrack::Err(traces)
            } else {
                Backtrack::Unmatch(traces)
            }
        );
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
    assert_eq!(
        Backtrack::check(true, span(1), undefined("failed")),
        Backtrack::Ok(())
    );
}

#[test]
fn test_deterministic_choice_reports_second_success_even_for_equal_values() {
    let mut visited = Vec::new();
    let result = choose_deterministic(["unmatch", "first", "second", "later"], |id| {
        visited.push(*id);
        if *id == "unmatch" {
            Backtrack::Unmatch(vec![])
        } else {
            Backtrack::Ok(42)
        }
    });
    assert_eq!(visited, ["unmatch", "first", "second"]);
    assert_eq!(result, Backtrack::Nondet("first", "second"));
}

#[test]
fn test_fatal_error_after_success_wins_and_stops_evaluation() {
    let mut visited = Vec::new();
    let result = choose_deterministic([0, 1, 2], |index| {
        visited.push(*index);
        match index {
            0 => Backtrack::Ok(7),
            1 => Backtrack::err(Span::default(), undefined("fatal")),
            _ => panic!("evaluated after fatal error"),
        }
    });
    assert_eq!(visited, [0, 1]);
    assert_eq!(
        result,
        Backtrack::Err(vec![Error {
            span: Span::default(),
            kind: Box::new(undefined("fatal")),
            children: vec![]
        }])
    );
}

#[test]
fn test_unique_success_survives_later_unmatches() {
    let mut visited = Vec::new();
    let result = choose_deterministic([0, 1, 2], |index| {
        visited.push(*index);
        if *index == 1 {
            Backtrack::Ok(7)
        } else {
            Backtrack::unmatch(Span::default(), undefined(format!("miss {index}")))
        }
    });
    assert_eq!(visited, [0, 1, 2]);
    assert_eq!(result, Backtrack::Ok(7));
}

#[test]
fn test_no_success_preserves_unmatch_order_and_fatal_precedence() {
    let result: Backtrack<(), _> = choose_deterministic(["a", "b"], |id| {
        Backtrack::unmatch(Span::default(), undefined(*id))
    });
    let traces: Vec<_> = ["a", "b"]
        .into_iter()
        .map(|message| Error {
            span: Span::default(),
            kind: Box::new(undefined(message)),
            children: vec![],
        })
        .collect();
    assert_eq!(result, Backtrack::Unmatch(traces));
    let result: Backtrack<(), _> =
        choose_deterministic(["miss", "fatal", "later"], |id| match *id {
            "miss" => Backtrack::unmatch(Span::default(), undefined("miss")),
            "fatal" => Backtrack::err(Span::default(), undefined("fatal")),
            _ => panic!("evaluated after fatal error"),
        });
    assert_eq!(
        result,
        Backtrack::Err(vec![Error {
            span: Span::default(),
            kind: Box::new(undefined("fatal")),
            children: vec![]
        }])
    );
    let empty: Backtrack<(), ()> = choose_deterministic([], |_| panic!("empty choice evaluated"));
    assert_eq!(empty, Backtrack::Unmatch(vec![]));
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

#[test]
fn test_nondeterminism_does_not_create_error_context() {
    let result = Backtrack::<(), usize>::Nondet(2, 4).nest(span(1), || {
        panic!("nondeterministic outcome created an error")
    });
    assert_eq!(result, Backtrack::<(), usize>::Nondet(2, 4));
}
