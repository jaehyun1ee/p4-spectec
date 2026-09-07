use p4spec_rust::{
    interp::al::backtrack::{Backtrack, FailTrace, choose_sequential},
    lang::common::source::{Position, Span},
};

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
                0 => Backtrack::unmatch(span(1), "first"),
                1 if fatal => Backtrack::err(span(2), "fatal"),
                _ => Backtrack::Ok(42),
            }
        });
        assert_eq!(visited, [0, 1]);
        let expected = if fatal {
            Backtrack::err(span(2), "fatal")
        } else {
            Backtrack::Ok(42)
        };
        assert_eq!(result, expected);
    }
}

#[test]
fn test_all_unmatched_candidates_keep_trace_order() {
    let result: Backtrack<()> = choose_sequential([1, 2], |line| {
        Backtrack::unmatch(span(*line), format!("candidate {line}"))
    });
    assert_eq!(
        result,
        Backtrack::Unmatch(vec![
            FailTrace {
                span: span(1),
                message: "candidate 1".into(),
                children: vec![]
            },
            FailTrace {
                span: span(2),
                message: "candidate 2".into(),
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
            Backtrack::err(span(2), "leaf")
        } else {
            Backtrack::unmatch(span(2), "leaf")
        };
        let result = result.nest(span(1), || "call".into());
        let traces = vec![FailTrace {
            span: span(1),
            message: "call".into(),
            children: vec![FailTrace {
                span: span(2),
                message: "leaf".into(),
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
        Backtrack::Ok(7).nest(span(1), || panic!("success formatted a failure")),
        Backtrack::Ok(7)
    );
    assert_eq!(
        Backtrack::<()>::Unmatch(vec![]).nest(span(1), || "call".into()),
        Backtrack::Unmatch(vec![FailTrace {
            span: span(1),
            message: "call".into(),
            children: vec![]
        }])
    );
}

#[test]
fn test_binding_short_circuits_both_failure_classes() {
    for result in [
        Backtrack::<usize>::err(span(1), "fatal"),
        Backtrack::unmatch(span(2), "mismatch"),
    ] {
        let expected = result.clone();
        assert_eq!(result.and_then(|_| panic!("failure continued")), expected);
    }
    assert_eq!(
        Backtrack::Ok(3).and_then(|n| Backtrack::Ok(n + 1)),
        Backtrack::Ok(4)
    );
    assert_eq!(
        Backtrack::check(false, span(1), "failed"),
        Backtrack::err(span(1), "failed")
    );
    assert_eq!(Backtrack::check(true, span(1), "failed"), Backtrack::Ok(()));
}
