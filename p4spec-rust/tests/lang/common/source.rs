use std::rc::Rc;

use p4spec_rust::lang::{
    common::source::{NotePhrase, Position, Span},
    traits::at::At,
};

#[test]
fn test_default_spans_share_file_names_and_preserve_positions() {
    let span_l = Span::default();
    let span_r = Span::default();
    assert_eq!(span_l, Span::new(Position::new("", 0, 0), Position::new("", 0, 0)));
    assert!(Rc::ptr_eq(&span_l.left.file, &span_r.left.file));
    assert!(Rc::ptr_eq(&span_l.right.file, &span_r.right.file));
    assert_eq!(span_l.to_string(), "");
}

#[test]
fn test_at_preserves_aggregate_endpoints_and_default_spans() {
    let span_a = Span::new(Position::new("a", 3, 2), Position::new("a", 5, 4));
    let span_b = Span::new(Position::new("a", 1, 7), Position::new("a", 4, 9));
    let span_c = Span::new(Position::new("b", 1, 0), Position::new("b", 2, 0));
    for (spans, span_expect) in [
        (vec![], Span::default()),
        (vec![span_a.clone()], span_a.clone()),
        (
            vec![span_a.clone(), span_b.clone()],
            Span::new(span_b.left.clone(), span_a.right.clone()),
        ),
        (
            vec![span_b.clone(), span_a.clone()],
            Span::new(span_b.left.clone(), span_a.right.clone()),
        ),
        (
            vec![span_a.clone(), Span::default()],
            Span::new(Position::default(), span_a.right.clone()),
        ),
        (
            vec![span_c.clone(), span_a.clone()],
            Span::new(span_a.left.clone(), span_c.right.clone()),
        ),
    ] {
        assert_eq!(Span::over(&spans), span_expect);
        assert_eq!(spans.at(), span_expect);
        assert_eq!(Span::over_iter(spans), span_expect);
    }
}

#[test]
fn test_at_reads_stored_phrase_span_through_shared_containers() {
    let span_inner = Span::new(Position::new("source", 2, 0), Position::new("source", 2, 1));
    let span_outer = Span::new(Position::new("source", 1, 0), Position::new("source", 3, 0));
    let phrase: NotePhrase<Span> = p4spec_rust::note_phrase!(
        node: span_inner.clone(), note: (), span: span_outer.clone(),
    );
    let phrases = [Rc::new(Box::new(phrase))];
    let phrases_borrowed = [&phrases[0]];
    assert_eq!(phrases.at(), span_outer);
    assert_eq!(phrases_borrowed.at(), span_outer);
    assert_eq!(phrases[0].node.at(), span_inner);
}
