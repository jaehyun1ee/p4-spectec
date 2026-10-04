use std::rc::Rc;

use p4spectec::{
    lang::{
        common::source::Span,
        data::value::{Arena, make},
    },
    phrase,
    runtime::envs::interp::shared::frame::{Frame, FrameLayout},
};

#[test]
fn unsetting_a_slot_preserves_other_slots_and_cloned_frames() {
    let mut layout = FrameLayout::default();
    let slot_x = layout
        .resolve_id(phrase!(node: "x".into(), span: Span::default()))
        .slot;
    let slot_y = layout
        .resolve_id(phrase!(node: "y".into(), span: Span::default()))
        .slot;
    let mut frame = Frame::new(Rc::new(layout));
    let mut arena = Arena::new();
    let value = make::nat(&mut arena, 7_u64.into(), Span::default()).unwrap();
    frame.set(slot_x, value);
    frame.set(slot_y, value);
    let frame_before = frame.clone();
    frame.unset(slot_x);
    assert_eq!(frame.get(slot_x), None);
    assert_eq!(frame.get(slot_y), Some(&value));
    assert_eq!(frame_before.get(slot_x), Some(&value));
    frame.set(slot_x, value);
    assert_eq!(frame.get(slot_x), Some(&value));
}
