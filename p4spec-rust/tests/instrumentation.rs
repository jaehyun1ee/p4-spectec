use std::{cell::RefCell, rc::Rc};

use p4spec_rust::{
    interp::instrumentation::{Handler, Hook},
    lang::common::source::Span,
    phrase,
    runtime::value::{ValueRef, make},
};

struct RecordingHandler {
    events: Rc<RefCell<Vec<String>>>,
}

impl Handler for RecordingHandler {
    fn backup(&mut self) {
        self.events.borrow_mut().push("backup".to_owned());
    }

    fn on_value(&mut self, _value: &ValueRef) {
        self.events.borrow_mut().push("value".to_owned());
    }

    fn on_rel_enter(&mut self, id: &p4spec_rust::lang::il::ast::Id, values: &[ValueRef]) {
        self.events
            .borrow_mut()
            .push(format!("rel:{}:{}", id.node, values.len()));
    }
}

#[test]
fn hooks_dispatch_in_registration_and_event_order() {
    let events = Rc::new(RefCell::new(Vec::new()));
    let mut hook = Hook::new();
    hook.register(Box::new(RecordingHandler {
        events: Rc::clone(&events),
    }));
    let value = make::bool(true, Span::default());

    hook.backup();
    hook.on_value(&value);
    hook.on_rel_enter(
        &phrase!(node: "R".to_owned(), span: Span::default()),
        &[value],
    );

    assert!(hook.is_active());
    assert_eq!(
        &*events.borrow(),
        &[
            "backup".to_owned(),
            "value".to_owned(),
            "rel:R:1".to_owned(),
        ]
    );
}

#[test]
fn empty_hook_is_a_no_op() {
    let mut hook = Hook::new();
    hook.on_value(&make::bool(true, Span::default()));
    hook.finish();
    assert!(!hook.is_active());
}
