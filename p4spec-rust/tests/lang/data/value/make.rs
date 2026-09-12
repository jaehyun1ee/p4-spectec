//! Value constructor annotation tests

use super::span;
use p4spec_rust::lang::{
    data::{
        typ,
        value::{ValueArena, get, make},
    },
    xl::num::{Natural, Number},
};

#[test]
fn test_constructors_preserve_runtime_type_and_span() {
    let mut arena = ValueArena::new();
    let value_span = span("program.p4", 4);
    let value = make::num(
        &mut arena,
        Number::Nat(Natural::from(7_u64)),
        value_span.clone(),
    )
    .unwrap();

    assert_eq!(arena.span(&value), &value_span);
    assert_eq!(arena.typ(&value).as_ref(), &typ::make::nat().node);
    assert_eq!(
        get::num(&arena, &value),
        Ok(&Number::Nat(Natural::from(7_u64)))
    );
}

#[test]
fn test_external_serde_state_roundtrip_and_replacement_preserve_annotations() {
    use serde::{Deserialize, Serialize};
    use serde_json::json;

    #[derive(Debug, PartialEq, Serialize, Deserialize)]
    enum Status {
        Active,
        Paused,
    }
    #[derive(Debug, PartialEq, Serialize, Deserialize)]
    struct State {
        count: u64,
        status: Status,
    }

    let mut arena = ValueArena::new();
    let typ = std::rc::Rc::new(typ::TypKind::Text);
    let span_state = span("state.p4", 17);
    let state = State {
        count: u64::MAX - 1,
        status: Status::Active,
    };
    let value = make::external(
        &mut arena,
        typ.clone(),
        serde_json::to_value(&state).unwrap(),
        span_state.clone(),
    )
    .unwrap();
    let mut state: State =
        serde_json::from_value(get::external(&arena, &value).unwrap().clone()).unwrap();
    assert_eq!(
        state,
        State {
            count: u64::MAX - 1,
            status: Status::Active
        }
    );
    state.count += 1;
    state.status = Status::Paused;
    let value_updated = make::external(
        &mut arena,
        typ.clone(),
        serde_json::to_value(&state).unwrap(),
        span_state.clone(),
    )
    .unwrap();
    assert_ne!(arena.canon_id(&value), arena.canon_id(&value_updated));
    assert_eq!(
        get::external(&arena, &value).unwrap(),
        &json!({"count": u64::MAX - 1, "status": "Active"})
    );
    assert_eq!(
        serde_json::from_value::<State>(get::external(&arena, &value_updated).unwrap().clone())
            .unwrap(),
        state
    );
    assert_eq!(arena.typ(&value_updated), &typ);
    assert_eq!(arena.span(&value_updated), &span_state);
    for json in [
        json!(null),
        json!({"status":"Active"}),
        json!({"count":-1,"status":"Active"}),
        json!({"count":0,"status":"unknown"}),
    ] {
        let value = make::external(&mut arena, typ.clone(), json, span_state.clone()).unwrap();
        assert!(
            serde_json::from_value::<State>(get::external(&arena, &value).unwrap().clone())
                .is_err()
        );
    }
}
