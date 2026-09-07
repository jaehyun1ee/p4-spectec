use std::rc::Rc;

use p4spec_rust::{
    interp::al::{
        assignment::{assign_args, assign_exp, assign_exps, is_iter_var_exp},
        backtrack::Backtrack,
        context::{Context, Scope, Spec},
    },
    lang::{
        al::ast,
        common::{
            Variable,
            source::{Position, Span},
        },
        data::{
            typ,
            value::{Value, get, make},
        },
    },
    note_phrase, phrase,
};

fn span(line: i64) -> Span {
    Span::new(
        Position::new("assignment.watsup", line, 0),
        Position::new("assignment.watsup", line, 1),
    )
}
fn id(name: &str) -> ast::Id {
    phrase!(node: name.to_owned(), span: span(3))
}
fn exp(node: ast::ExpKind) -> ast::Exp {
    note_phrase!(node: node, note: typ::make::bool().node, span: span(4))
}
fn var_exp(name: &str) -> ast::Exp {
    exp(ast::ExpKind::Var(id(name)))
}
fn var(name: &str, iters: Vec<ast::Iter>) -> ast::Var {
    ast::Var {
        id: id(name),
        typ: phrase!(node: typ::make::bool().node, span: span(5)),
        iters,
    }
}
fn value(value: bool) -> Rc<Value> {
    make::bool(value, span(8))
}
fn ok<T: std::fmt::Debug>(result: Backtrack<T>) -> T {
    match result {
        Backtrack::Ok(value) => value,
        other => panic!("{other:?}"),
    }
}
fn binding(ctx: &Context, name: &str, iters: Vec<ast::Iter>) -> Rc<Value> {
    Rc::clone(ctx.find_value(&Variable::new(id(name), iters)).unwrap())
}
fn iter(exp_inner: ast::Exp, iter: ast::Iter, vars: Vec<ast::Var>) -> ast::Exp {
    exp(ast::ExpKind::Iter(Box::new(exp_inner), (iter, vars)))
}
fn tuple(values: Vec<Rc<Value>>) -> Rc<Value> {
    make::tuple(&typ::make::bool(), values, span(8))
}
fn list(values: Vec<Rc<Value>>) -> Rc<Value> {
    make::list(&typ::make::list(typ::make::bool()), values, span(8))
}

#[test]
fn test_iterated_variable_fast_path_preserves_identity_and_path() {
    let exp_inner = iter(var_exp("x"), ast::Iter::Opt, vec![var("x", vec![])]);
    let exp = iter(
        exp_inner.clone(),
        ast::Iter::List,
        vec![var("x", vec![ast::Iter::Opt])],
    );
    assert_eq!(
        is_iter_var_exp(&exp).unwrap().iters,
        vec![ast::Iter::Opt, ast::Iter::List]
    );
    let value = list(vec![]);
    let ctx = ok(assign_exp(&Context::new(), &exp, Rc::clone(&value)));
    assert!(Rc::ptr_eq(
        &value,
        &binding(&ctx, "x", vec![ast::Iter::Opt, ast::Iter::List])
    ));
    assert!(
        ctx.find_value_opt(&Variable::new(id("x"), vec![]))
            .is_none()
    );
    assert!(is_iter_var_exp(&iter(exp_inner, ast::Iter::List, vec![var("x", vec![])])).is_none());
}

#[test]
fn test_list_assignment_collects_rows_without_leaking_scalar_bindings() {
    let mut ctx = Context::new();
    ctx.add_value(Variable::new(id("x"), vec![]), value(false));
    let exp = iter(
        exp(ast::ExpKind::Tuple(vec![var_exp("x"), var_exp("y")])),
        ast::Iter::List,
        vec![var("x", vec![]), var("y", vec![])],
    );
    let ctx_result = ok(assign_exp(
        &ctx,
        &exp,
        list(vec![
            tuple(vec![value(true), value(false)]),
            tuple(vec![value(false), value(true)]),
        ]),
    ));
    let xs = binding(&ctx_result, "x", vec![ast::Iter::List]);
    let ys = binding(&ctx_result, "y", vec![ast::Iter::List]);
    assert_eq!(xs.span, Span::default());
    assert_eq!(ys.span, Span::default());
    assert_eq!(
        get::list(&xs)
            .unwrap()
            .iter()
            .map(|v| get::bool(v).unwrap())
            .collect::<Vec<_>>(),
        vec![true, false]
    );
    assert_eq!(
        get::list(&ys)
            .unwrap()
            .iter()
            .map(|v| get::bool(v).unwrap())
            .collect::<Vec<_>>(),
        vec![false, true]
    );
    assert!(!get::bool(&binding(&ctx_result, "x", vec![])).unwrap());
    assert!(
        ctx_result
            .find_value_opt(&Variable::new(id("y"), vec![]))
            .is_none()
    );
    assert!(
        ctx.find_value_opt(&Variable::new(id("x"), vec![ast::Iter::List]))
            .is_none()
    );
}

#[test]
fn test_list_rows_cannot_collect_unassigned_outer_values() {
    let mut ctx = Context::new();
    ctx.add_value(Variable::new(id("missing"), vec![]), value(true));
    let exp = iter(
        exp(ast::ExpKind::Tuple(vec![var_exp("x")])),
        ast::Iter::List,
        vec![var("missing", vec![])],
    );
    let Backtrack::Err(traces) = assign_exp(&ctx, &exp, list(vec![tuple(vec![value(false)])]))
    else {
        panic!("expected missing row binding")
    };
    assert_eq!(traces[0].span, span(3));
}

#[test]
fn test_optional_assignment_retains_inner_bindings_and_collects_none() {
    let exp = iter(
        exp(ast::ExpKind::Tuple(vec![var_exp("x")])),
        ast::Iter::Opt,
        vec![var("x", vec![])],
    );
    let typ = typ::make::opt(typ::make::bool());
    let ctx = ok(assign_exp(
        &Context::new(),
        &exp,
        make::opt(&typ, Some(tuple(vec![value(true)])), span(8)),
    ));
    assert!(get::bool(&binding(&ctx, "x", vec![])).unwrap());
    assert!(
        get::bool(
            get::opt(&binding(&ctx, "x", vec![ast::Iter::Opt]))
                .unwrap()
                .unwrap()
        )
        .unwrap()
    );
    assert_eq!(
        binding(&ctx, "x", vec![ast::Iter::Opt]).span,
        Span::default()
    );
    let ctx = ok(assign_exp(&ctx, &exp, make::opt(&typ, None, span(8))));
    assert_eq!(
        binding(&ctx, "x", vec![ast::Iter::Opt]).span,
        Span::default()
    );
    assert!(
        get::opt(&binding(&ctx, "x", vec![ast::Iter::Opt]))
            .unwrap()
            .is_none()
    );
    assert!(get::bool(&binding(&ctx, "x", vec![])).unwrap());
}

#[test]
fn test_cons_tail_preserves_value_type_with_default_span() {
    let exp = exp(ast::ExpKind::Cons(
        Box::new(var_exp("h")),
        Box::new(var_exp("t")),
    ));
    let value = list(vec![value(true), value(false)]);
    let ctx = ok(assign_exp(&Context::new(), &exp, Rc::clone(&value)));
    let tail = binding(&ctx, "t", vec![]);
    assert_eq!(tail.note, value.note);
    assert_eq!(tail.span, Span::default());
    assert!(Rc::ptr_eq(
        &binding(&ctx, "h", vec![]),
        &get::list(&value).unwrap()[0]
    ));
    assert!(!get::bool(&get::list(&tail).unwrap()[0]).unwrap());
    assert!(matches!(
        assign_exp(&ctx, &exp, list(vec![])),
        Backtrack::Err(_)
    ));
}

#[test]
fn test_assignment_errors_are_fatal_and_located() {
    let exps = vec![
        var_exp("x"),
        note_phrase!(node: ast::ExpKind::Var(id("y")), note: typ::make::bool().node, span: span(9)),
    ];
    let Backtrack::Err(traces) = assign_exps(&Context::new(), &exps, &[]) else {
        panic!("expected arity error")
    };
    assert_eq!(traces[0].span, Span::over(&[span(4), span(9)]));
    let exp = exp(ast::ExpKind::Opt(None));
    let Backtrack::Err(traces) = assign_exp(
        &Context::new(),
        &exp,
        make::opt(
            &typ::make::opt(typ::make::bool()),
            Some(value(true)),
            span(8),
        ),
    ) else {
        panic!("expected optionality error")
    };
    assert_eq!(traces[0].span, exp.span);
}

#[test]
fn test_function_argument_copies_caller_definition_without_caller_values() {
    let spec = Spec::load(vec![]).unwrap();
    let mut caller = Context::new();
    let func = ast::MetaFuncDef::Extern(ast::ExternFunc {
        id: id("f"),
        tparams: vec![],
        params: vec![],
        typ: typ::make::bool(),
        hints: vec![],
    });
    caller.add_func(&spec, id("f"), func.clone()).unwrap();
    caller.add_value(Variable::new(id("secret"), vec![]), value(true));
    let arg = phrase!(node: ast::ArgKind::Def(id("alias")), span: span(4));
    let func_value = make::func(id("f"), vec![], vec![], typ::make::bool(), span(8));
    let callee = ok(assign_args(
        &spec,
        &caller,
        &Context::new(),
        &[arg],
        &[func_value],
    ));
    assert_eq!(
        callee.find_func(&spec, &id("alias")).unwrap(),
        (Scope::Local, &func)
    );
    assert!(
        callee
            .find_value_opt(&Variable::new(id("secret"), vec![]))
            .is_none()
    );
    assert!(caller.find_func_opt(&spec, &id("alias")).is_none());
}

#[test]
fn test_case_and_struct_assignments_follow_argument_order() {
    use p4spec_rust::lang::common::notation::{atom::Atom, mixfix::Mixfix};
    let atom = |name: &str| phrase!(node: Atom::keyword(name), span: span(3));
    let case_exp = exp(ast::ExpKind::Case(Box::new(Mixfix::Seq(vec![
        Mixfix::Atom(atom("LEFT")),
        Mixfix::Arg(var_exp("x")),
        Mixfix::Arg(var_exp("y")),
    ]))));
    let case_value = make::case_(
        &typ::make::bool(),
        Mixfix::Seq(vec![
            Mixfix::Atom(atom("RIGHT")),
            Mixfix::Arg(value(true)),
            Mixfix::Arg(value(false)),
        ]),
        span(8),
    );
    let struct_exp = exp(ast::ExpKind::Str(vec![
        (atom("a"), var_exp("x")),
        (atom("b"), var_exp("y")),
    ]));
    let struct_value = make::structure(
        &typ::make::bool(),
        vec![(atom("b"), value(true)), (atom("a"), value(false))],
        span(8),
    );
    for (exp, value) in [(case_exp, case_value), (struct_exp, struct_value)] {
        let ctx = ok(assign_exp(&Context::new(), &exp, value));
        assert!(get::bool(&binding(&ctx, "x", vec![])).unwrap());
        assert!(!get::bool(&binding(&ctx, "y", vec![])).unwrap());
    }
}

#[test]
fn test_empty_iteration_creates_empty_collections_for_every_binding() {
    let exp = iter(
        exp(ast::ExpKind::Tuple(vec![var_exp("x"), var_exp("y")])),
        ast::Iter::List,
        vec![var("x", vec![]), var("y", vec![])],
    );
    let ctx = ok(assign_exp(&Context::new(), &exp, list(vec![])));
    for name in ["x", "y"] {
        assert!(
            get::list(&binding(&ctx, name, vec![ast::Iter::List]))
                .unwrap()
                .is_empty()
        );
    }
}
