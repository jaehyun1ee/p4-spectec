use super::*;
use p4spec_rust::lang::traits::has_call::HasCall;

#[test]
fn test_update_path_call_is_detected() {
    let exp_call = exp(ast::ExpKind::Call(id("index"), Vec::new(), Vec::new()));
    let path_root = p4spec_rust::note_phrase! {
        node: ast::PathKind::Root,
        note: ast::TypKind::Bool,
        span: Span::default(),
    };
    let path = p4spec_rust::note_phrase! {
        node: ast::PathKind::Idx(Box::new(path_root), Box::new(exp_call)),
        note: ast::TypKind::Bool,
        span: Span::default(),
    };
    let exp_update =
        exp(ast::ExpKind::Upd(Box::new(var("base")), Box::new(path), Box::new(var("field"))));

    assert!(exp_update.has_call());
    assert!(!var("plain").has_call());
}
