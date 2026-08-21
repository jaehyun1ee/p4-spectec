use p4spec_rust::{
    sim::ebpf::{CounterArray, transform_stf},
    wire::sim_suite::{StfAction, StfStmt},
};

fn action(name: &str) -> StfAction {
    StfAction {
        name: name.to_owned(),
        args: Vec::new(),
    }
}

#[test]
fn ebpf_stf_names_match_the_ocaml_transform() {
    let transformed = transform_stf(StfStmt::SetDefault {
        name: "pipe_c1_table".to_owned(),
        action: action("pipe_c1_action_NoAction"),
    });
    let StfStmt::SetDefault {
        name,
        action: transformed_action,
    } = transformed
    else {
        panic!("expected set-default statement");
    };
    assert_eq!(name, "main.filt.c1.table");
    assert_eq!(transformed_action.name, "actionNoAction");

    let transformed = transform_stf(StfStmt::SetDefault {
        name: "pipe_table".to_owned(),
        action: action("pipe_action"),
    });
    let StfStmt::SetDefault {
        name,
        action: transformed_action,
    } = transformed
    else {
        panic!("expected set-default statement");
    };
    assert_eq!(name, "main.filt.table");
    assert_eq!(transformed_action.name, "action");

    let transformed = transform_stf(StfStmt::SetDefault {
        name: "pipe".to_owned(),
        action: action("pipe.NoAction"),
    });
    let StfStmt::SetDefault {
        name,
        action: transformed_action,
    } = transformed
    else {
        panic!("expected set-default statement");
    };
    assert_eq!(name, "main.filt");
    assert_eq!(transformed_action.name, "NoAction");
}

#[test]
fn counter_array_updates_in_range_only() {
    let mut counters = CounterArray::new(3, true);
    assert_eq!(counters.values(), &[0, 0, 0]);

    counters.increment(1);
    counters.add(1, 4);
    counters.add(9, 10);

    assert_eq!(counters.values(), &[0, 5, 0]);
}
