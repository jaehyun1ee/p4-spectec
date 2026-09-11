use std::rc::Rc;

use p4spec_rust::lang::common::source::{Position, Span};

#[test]
fn test_default_spans_share_file_names_and_preserve_positions() {
    let span_l = Span::default();
    let span_r = Span::default();
    assert_eq!(
        span_l,
        Span::new(Position::new("", 0, 0), Position::new("", 0, 0))
    );
    assert!(Rc::ptr_eq(&span_l.left.file, &span_r.left.file));
    assert!(Rc::ptr_eq(&span_l.right.file, &span_r.right.file));
    assert_eq!(span_l.to_string(), "");
}
