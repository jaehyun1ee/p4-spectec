use p4spec_rust::{
    interp::al::{
        backtrack::{Backtrack, FailTrace},
        nondet::{BacktrackDet, choose_deterministic},
    },
    lang::common::source::Span,
};

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
    assert_eq!(result, BacktrackDet::Nondet("first", "second"));
}

#[test]
fn test_fatal_error_after_success_wins_and_stops_evaluation() {
    let mut visited = Vec::new();
    let result = choose_deterministic([0, 1, 2], |index| {
        visited.push(*index);
        match index {
            0 => Backtrack::Ok(7),
            1 => Backtrack::err(Span::default(), "fatal"),
            _ => panic!("evaluated after fatal error"),
        }
    });
    assert_eq!(visited, [0, 1]);
    assert_eq!(
        result,
        BacktrackDet::Err(vec![FailTrace {
            span: Span::default(),
            message: "fatal".into(),
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
            Backtrack::unmatch(Span::default(), format!("miss {index}"))
        }
    });
    assert_eq!(visited, [0, 1, 2]);
    assert_eq!(result, BacktrackDet::Ok(7));
}

#[test]
fn test_no_success_preserves_unmatch_order_and_fatal_precedence() {
    let result: BacktrackDet<(), _> =
        choose_deterministic(["a", "b"], |id| Backtrack::unmatch(Span::default(), *id));
    let traces: Vec<_> = ["a", "b"]
        .into_iter()
        .map(|message| FailTrace {
            span: Span::default(),
            message: message.into(),
            children: vec![],
        })
        .collect();
    assert_eq!(result, BacktrackDet::Unmatch(traces));
    let result: BacktrackDet<(), _> =
        choose_deterministic(["miss", "fatal", "later"], |id| match *id {
            "miss" => Backtrack::unmatch(Span::default(), "miss"),
            "fatal" => Backtrack::err(Span::default(), "fatal"),
            _ => panic!("evaluated after fatal error"),
        });
    assert_eq!(
        result,
        BacktrackDet::Err(vec![FailTrace {
            span: Span::default(),
            message: "fatal".into(),
            children: vec![]
        }])
    );
    let empty: BacktrackDet<(), ()> =
        choose_deterministic([], |_| panic!("empty choice evaluated"));
    assert_eq!(empty, BacktrackDet::Unmatch(vec![]));
}
