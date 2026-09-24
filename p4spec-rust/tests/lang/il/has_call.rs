use super::*;
use p4spec_rust::lang::traits::has_call::HasCall;

fn exp(kind: ast::ExpKind) -> ast::Exp {
    p4spec_rust::note_phrase! {
        node: kind,
        note: ast::TypKind::Bool,
        span: Span::default(),
    }
}

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
        exp(ast::ExpKind::Upd(Box::new(id_exp("base")), Box::new(path), Box::new(id_exp("field"))));

    let exps_call = exp_update.nested_call();
    assert_eq!(exps_call.len(), 1);
    let ast::ExpKind::Upd(_, path, _) = &exp_update.node else { unreachable!() };
    let ast::PathKind::Idx(_, exp_idx) = &path.node else { unreachable!() };
    assert!(std::ptr::eq(exps_call[0], exp_idx.as_ref()));
    assert!(exp_update.has_call());
    assert!(!id_exp("plain").has_call());
}

/// Builds a call with a distinct source location.
fn call(name: &str, line: usize, args: Vec<ast::Arg>) -> ast::Exp {
    let mut exp_call = exp(ast::ExpKind::Call(id(name), vec![], args));
    let pos = Position::new("calls.watsup", line, 0);
    exp_call.span = Span::new(pos.clone(), pos);
    exp_call
}

#[test]
fn test_nested_calls_retain_original_expressions_in_preorder() {
    let exp_inner = call("g", 20, vec![]);
    let arg_inner = p4spec_rust::phrase!(
        node: ast::ArgKind::Exp(Box::new(exp_inner)), span: Span::default());
    let arg_func = p4spec_rust::phrase!(
        node: ast::ArgKind::Def(id("named")), span: Span::default());
    let exp_outer = call("f", 10, vec![arg_func, arg_inner]);
    let exp_tuple = exp(ast::ExpKind::Tuple(vec![exp_outer, call("h", 30, vec![])]));
    let exps_call = exp_tuple.nested_call();
    assert_eq!(
        exps_call
            .iter()
            .map(|exp| exp.span.left.line)
            .collect::<Vec<_>>(),
        [10, 20, 30]
    );
    let ast::ExpKind::Tuple(exps) = &exp_tuple.node else { unreachable!() };
    let ast::ExpKind::Call(_, _, args) = &exps[0].node else { unreachable!() };
    let ast::ArgKind::Exp(exp_inner) = &args[1].node else { unreachable!() };
    assert!(std::ptr::eq(exps_call[0], &exps[0]));
    assert!(std::ptr::eq(exps_call[1], exp_inner.as_ref()));
    assert!(std::ptr::eq(exps_call[2], &exps[1]));
    assert!(exp_tuple.has_call());
    assert!(id_exp("plain").nested_call().is_empty());
    assert!(!id_exp("plain").has_call());
}
