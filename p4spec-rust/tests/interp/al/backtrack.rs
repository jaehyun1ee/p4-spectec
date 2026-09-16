use p4spec_rust::interp::al::backtrack::{choose_deterministic, choose_sequential};
use p4spec_rust::interp::shared::error::ContextErrorKind;
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
        let expected =
            if fatal { Backtrack::err(span(2), undefined("fatal")) } else { Backtrack::Ok(42) };
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
            Error { span: span(1), kind: Box::new(undefined("candidate 1")), children: vec![] },
            Error { span: span(2), kind: Box::new(undefined("candidate 2")), children: vec![] },
        ])
    );
    let empty: Backtrack<()> = choose_sequential([], |_: &()| panic!("empty choice evaluated"));
    assert_eq!(empty, Backtrack::Unmatch(vec![]));
}

#[test]
fn test_deterministic_choice_reports_second_success_even_for_equal_values() {
    let mut visited = Vec::new();
    let result = choose_deterministic(
        ["unmatch", "first", "second", "later"],
        |id| {
            visited.push(*id);
            if *id == "unmatch" { Backtrack::Unmatch(vec![]) } else { Backtrack::Ok(42) }
        },
        |candidate_a, candidate_b| {
            Error::new(undefined(format!("{candidate_a}, {candidate_b}")), Span::default())
        },
    );
    assert_eq!(visited, ["unmatch", "first", "second"]);
    assert_eq!(
        result,
        Backtrack::Err(vec![Error::new(undefined("first, second"), Span::default())])
    );
}

#[test]
fn test_fatal_error_after_success_wins_and_stops_evaluation() {
    let mut visited = Vec::new();
    let result = choose_deterministic(
        [0, 1, 2],
        |index| {
            visited.push(*index);
            match index {
                0 => Backtrack::Ok(7),
                1 => Backtrack::err(Span::default(), undefined("fatal")),
                _ => panic!("evaluated after fatal error"),
            }
        },
        |candidate_a, candidate_b| {
            Error::new(undefined(format!("{candidate_a}, {candidate_b}")), Span::default())
        },
    );
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
    let result = choose_deterministic(
        [0, 1, 2],
        |index| {
            visited.push(*index);
            if *index == 1 {
                Backtrack::Ok(7)
            } else {
                Backtrack::unmatch(Span::default(), undefined(format!("miss {index}")))
            }
        },
        |candidate_a, candidate_b| {
            Error::new(undefined(format!("{candidate_a}, {candidate_b}")), Span::default())
        },
    );
    assert_eq!(visited, [0, 1, 2]);
    assert_eq!(result, Backtrack::Ok(7));
}

#[test]
fn test_no_success_preserves_unmatch_order_and_fatal_precedence() {
    let result: Backtrack<()> = choose_deterministic(
        ["a", "b"],
        |id| Backtrack::unmatch(Span::default(), undefined(*id)),
        |candidate_a, candidate_b| {
            Error::new(undefined(format!("{candidate_a}, {candidate_b}")), Span::default())
        },
    );
    let traces: Vec<_> = ["a", "b"]
        .into_iter()
        .map(|message| Error {
            span: Span::default(),
            kind: Box::new(undefined(message)),
            children: vec![],
        })
        .collect();
    assert_eq!(result, Backtrack::Unmatch(traces));
    let result: Backtrack<()> = choose_deterministic(
        ["miss", "fatal", "later"],
        |id| match *id {
            "miss" => Backtrack::unmatch(Span::default(), undefined("miss")),
            "fatal" => Backtrack::err(Span::default(), undefined("fatal")),
            _ => panic!("evaluated after fatal error"),
        },
        |candidate_a, candidate_b| {
            Error::new(undefined(format!("{candidate_a}, {candidate_b}")), Span::default())
        },
    );
    assert_eq!(
        result,
        Backtrack::Err(vec![Error {
            span: Span::default(),
            kind: Box::new(undefined("fatal")),
            children: vec![]
        }])
    );
    let empty: Backtrack<()> = choose_deterministic(
        [],
        |_| panic!("empty choice evaluated"),
        |_: (), _: ()| panic!("empty choice overlap"),
    );
    assert_eq!(empty, Backtrack::Unmatch(vec![]));
}

#[test]
fn test_unique_success_does_not_construct_nondeterminism_error() {
    let result = choose_deterministic(
        [1],
        |_| Backtrack::Ok(7),
        |_, _| panic!("unique candidate constructed a nondeterminism error"),
    );
    assert_eq!(result, Backtrack::Ok(7));
}
