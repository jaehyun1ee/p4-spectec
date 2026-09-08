use p4spec_rust::lang::data::value::ValueArena;

use p4spec_rust::{
    interp::al::{
        backtrack::Backtrack,
        context::{Context, Global, Scope},
        eval::assign::{assign_args, assign_exp, assign_exps},
        util::is_iter_var_exp,
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
fn value(arena: &mut ValueArena, value: bool) -> Value {
    make::bool(arena, value, span(8)).unwrap()
}
fn ok<T: std::fmt::Debug>(result: Backtrack<T>) -> T {
    match result {
        Backtrack::Ok(value) => value,
        other => panic!("{other:?}"),
    }
}
fn binding(ctx: &Context<'_>, name: &str, iters: Vec<ast::Iter>) -> Value {
    *ctx.find_value(&Variable::new(id(name), iters)).unwrap()
}
fn iter(exp_inner: ast::Exp, iter: ast::Iter, vars: Vec<ast::Var>) -> ast::Exp {
    exp(ast::ExpKind::Iter(Box::new(exp_inner), (iter, vars)))
}
fn tuple(arena: &mut ValueArena, values: Vec<Value>) -> Value {
    make::tuple(arena, &typ::make::bool(), values, span(8)).unwrap()
}
fn list(arena: &mut ValueArena, values: Vec<Value>) -> Value {
    make::list(arena, &typ::make::list(typ::make::bool()), values, span(8)).unwrap()
}

#[test]
fn test_iterated_variable_fast_path_preserves_identity_and_path() {
    let mut arena = ValueArena::new();
    let global = Global::load(vec![]).unwrap();
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
    let value = list(&mut arena, vec![]);
    let ctx = ok(assign_exp(&mut arena, &Context::new(&global), &exp, value));
    assert!(std::cmp::PartialEq::eq(
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
    let mut arena = ValueArena::new();
    let global = Global::load(vec![]).unwrap();
    let mut ctx = Context::new(&global);
    ctx.add_value(Variable::new(id("x"), vec![]), value(&mut arena, false));
    let exp = iter(
        exp(ast::ExpKind::Tuple(vec![var_exp("x"), var_exp("y")])),
        ast::Iter::List,
        vec![var("x", vec![]), var("y", vec![])],
    );
    let ctx_result = ok({
        let (value_arg0, value_arg1, value_arg2) = (&ctx, &exp, {
            let (value_arg0,) = (vec![
                {
                    let (value_arg0,) = (vec![value(&mut arena, true), value(&mut arena, false)],);
                    tuple(&mut arena, value_arg0)
                },
                {
                    let (value_arg0,) = (vec![value(&mut arena, false), value(&mut arena, true)],);
                    tuple(&mut arena, value_arg0)
                },
            ],);
            list(&mut arena, value_arg0)
        });
        assign_exp(&mut arena, value_arg0, value_arg1, value_arg2)
    });
    let xs = binding(&ctx_result, "x", vec![ast::Iter::List]);
    let ys = binding(&ctx_result, "y", vec![ast::Iter::List]);
    assert_eq!(*arena.span(&xs), Span::default());
    assert_eq!(*arena.span(&ys), Span::default());
    assert_eq!(
        get::list(&arena, &xs)
            .unwrap()
            .iter()
            .map(|v| get::bool(&arena, v).unwrap())
            .collect::<Vec<_>>(),
        vec![true, false]
    );
    assert_eq!(
        get::list(&arena, &ys)
            .unwrap()
            .iter()
            .map(|v| get::bool(&arena, v).unwrap())
            .collect::<Vec<_>>(),
        vec![false, true]
    );
    assert!(!get::bool(&arena, &binding(&ctx_result, "x", vec![])).unwrap());
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
    let mut arena = ValueArena::new();
    let global = Global::load(vec![]).unwrap();
    let mut ctx = Context::new(&global);
    ctx.add_value(
        Variable::new(id("missing"), vec![]),
        value(&mut arena, true),
    );
    let exp = iter(
        exp(ast::ExpKind::Tuple(vec![var_exp("x")])),
        ast::Iter::List,
        vec![var("missing", vec![])],
    );
    let Backtrack::Err(traces) = ({
        let (value_arg0, value_arg1, value_arg2) = (&ctx, &exp, {
            let (value_arg0,) = (vec![{
                let (value_arg0,) = (vec![value(&mut arena, false)],);
                tuple(&mut arena, value_arg0)
            }],);
            list(&mut arena, value_arg0)
        });
        assign_exp(&mut arena, value_arg0, value_arg1, value_arg2)
    }) else {
        panic!("expected missing row binding")
    };
    assert_eq!(traces[0].span, span(3));
}

#[test]
fn test_optional_assignment_retains_inner_bindings_and_collects_none() {
    let mut arena = ValueArena::new();
    let global = Global::load(vec![]).unwrap();
    let exp = iter(
        exp(ast::ExpKind::Tuple(vec![var_exp("x")])),
        ast::Iter::Opt,
        vec![var("x", vec![])],
    );
    let typ = typ::make::opt(typ::make::bool());
    let ctx = ok({
        let (value_arg0, value_arg1, value_arg2) = (
            &Context::new(&global),
            &exp,
            ({
                let (value_arg0, value_arg1, value_arg2) = (
                    &typ,
                    Some({
                        let (value_arg0,) = (vec![value(&mut arena, true)],);
                        tuple(&mut arena, value_arg0)
                    }),
                    span(8),
                );
                make::opt(&mut arena, value_arg0, value_arg1, value_arg2)
            })
            .unwrap(),
        );
        assign_exp(&mut arena, value_arg0, value_arg1, value_arg2)
    });
    assert!(get::bool(&arena, &binding(&ctx, "x", vec![])).unwrap());
    assert!(
        get::bool(
            &arena,
            get::opt(&arena, &binding(&ctx, "x", vec![ast::Iter::Opt]))
                .unwrap()
                .unwrap()
        )
        .unwrap()
    );
    assert_eq!(
        *arena.span(&binding(&ctx, "x", vec![ast::Iter::Opt])),
        Span::default()
    );
    let ctx = ok({
        let (value_arg0, value_arg1, value_arg2) = (
            &ctx,
            &exp,
            make::opt(&mut arena, &typ, None, span(8)).unwrap(),
        );
        assign_exp(&mut arena, value_arg0, value_arg1, value_arg2)
    });
    assert_eq!(
        *arena.span(&binding(&ctx, "x", vec![ast::Iter::Opt])),
        Span::default()
    );
    assert!(
        get::opt(&arena, &binding(&ctx, "x", vec![ast::Iter::Opt]))
            .unwrap()
            .is_none()
    );
    assert!(get::bool(&arena, &binding(&ctx, "x", vec![])).unwrap());
}

#[test]
fn test_cons_tail_preserves_value_type_with_default_span() {
    let mut arena = ValueArena::new();
    let global = Global::load(vec![]).unwrap();
    let exp = exp(ast::ExpKind::Cons(
        Box::new(var_exp("h")),
        Box::new(var_exp("t")),
    ));
    let value = {
        let (value_arg0,) = (vec![value(&mut arena, true), value(&mut arena, false)],);
        list(&mut arena, value_arg0)
    };
    let ctx = ok(assign_exp(&mut arena, &Context::new(&global), &exp, value));
    let tail = binding(&ctx, "t", vec![]);
    assert_eq!(arena.typ(&tail), arena.typ(&value));
    assert_eq!(*arena.span(&tail), Span::default());
    assert!(std::cmp::PartialEq::eq(
        &binding(&ctx, "h", vec![]),
        &get::list(&arena, &value).unwrap()[0]
    ));
    assert!(!get::bool(&arena, &get::list(&arena, &tail).unwrap()[0]).unwrap());
    assert!(matches!(
        {
            let (value_arg0, value_arg1, value_arg2) = (&ctx, &exp, list(&mut arena, vec![]));
            assign_exp(&mut arena, value_arg0, value_arg1, value_arg2)
        },
        Backtrack::Err(_)
    ));
}

#[test]
fn test_assignment_errors_are_fatal_and_located() {
    let mut arena = ValueArena::new();
    let global = Global::load(vec![]).unwrap();
    let exps = vec![
        var_exp("x"),
        note_phrase!(node: ast::ExpKind::Var(id("y")), note: typ::make::bool().node, span: span(9)),
    ];
    let Backtrack::Err(traces) = assign_exps(&mut arena, &Context::new(&global), &exps, &[]) else {
        panic!("expected arity error")
    };
    assert_eq!(traces[0].span, Span::over(&[span(4), span(9)]));
    let exp = exp(ast::ExpKind::Opt(None));
    let Backtrack::Err(traces) = ({
        let (value_arg0, value_arg1, value_arg2) = (
            &Context::new(&global),
            &exp,
            ({
                let (value_arg0, value_arg1, value_arg2) = (
                    &typ::make::opt(typ::make::bool()),
                    Some(value(&mut arena, true)),
                    span(8),
                );
                make::opt(&mut arena, value_arg0, value_arg1, value_arg2)
            })
            .unwrap(),
        );
        assign_exp(&mut arena, value_arg0, value_arg1, value_arg2)
    }) else {
        panic!("expected optionality error")
    };
    assert_eq!(traces[0].span, exp.span);
}

#[test]
fn test_function_argument_copies_caller_definition_without_caller_values() {
    let mut arena = ValueArena::new();
    let global = Global::load(vec![]).unwrap();
    let func = ast::MetaFuncDef::Extern(ast::ExternFunc {
        id: id("f"),
        tparams: vec![],
        params: vec![],
        typ: typ::make::bool(),
        hints: vec![],
    });
    let callee = {
        let global_caller = Global::load(vec![]).unwrap();
        let mut caller = Context::new(&global_caller);
        caller.add_func(id("f"), func.clone()).unwrap();
        caller.add_value(Variable::new(id("secret"), vec![]), value(&mut arena, true));
        let arg = phrase!(node: ast::ArgKind::Def(id("alias")), span: span(4));
        let func_value = make::func(
            &mut arena,
            id("f"),
            vec![],
            vec![],
            typ::make::bool(),
            span(8),
        )
        .unwrap();
        let callee = ok(assign_args(
            &mut arena,
            &caller,
            &Context::new(&global),
            &[arg],
            &[func_value],
        ));
        assert!(caller.find_func_opt(&id("alias")).is_none());
        callee
    };
    assert_eq!(
        callee.find_func(&id("alias")).unwrap(),
        (Scope::Local, &func)
    );
    assert!(
        callee
            .find_value_opt(&Variable::new(id("secret"), vec![]))
            .is_none()
    );
}

#[test]
fn test_case_and_struct_assignments_follow_argument_order() {
    let mut arena = ValueArena::new();
    let global = Global::load(vec![]).unwrap();
    use p4spec_rust::lang::common::notation::{atom::Atom, mixfix::Mixfix};
    let atom = |name: &str| phrase!(node: Atom::keyword(name), span: span(3));
    let case_exp = exp(ast::ExpKind::Case(Box::new(Mixfix::Seq(vec![
        Mixfix::Atom(atom("LEFT")),
        Mixfix::Arg(var_exp("x")),
        Mixfix::Arg(var_exp("y")),
    ]))));
    let case_value = ({
        let (value_arg0, value_arg1, value_arg2) = (
            &typ::make::bool(),
            Mixfix::Seq(vec![
                Mixfix::Atom(atom("RIGHT")),
                Mixfix::Arg(value(&mut arena, true)),
                Mixfix::Arg(value(&mut arena, false)),
            ]),
            span(8),
        );
        make::case_(&mut arena, value_arg0, value_arg1, value_arg2)
    })
    .unwrap();
    let struct_exp = exp(ast::ExpKind::Str(vec![
        (atom("a"), var_exp("x")),
        (atom("b"), var_exp("y")),
    ]));
    let struct_value = ({
        let (value_arg0, value_arg1, value_arg2) = (
            &typ::make::bool(),
            vec![
                (atom("b"), value(&mut arena, true)),
                (atom("a"), value(&mut arena, false)),
            ],
            span(8),
        );
        make::structure(&mut arena, value_arg0, value_arg1, value_arg2)
    })
    .unwrap();
    for (exp, value) in [(case_exp, case_value), (struct_exp, struct_value)] {
        let ctx = ok(assign_exp(&mut arena, &Context::new(&global), &exp, value));
        assert!(get::bool(&arena, &binding(&ctx, "x", vec![])).unwrap());
        assert!(!get::bool(&arena, &binding(&ctx, "y", vec![])).unwrap());
    }
}

#[test]
fn test_empty_iteration_creates_empty_collections_for_every_binding() {
    let mut arena = ValueArena::new();
    let global = Global::load(vec![]).unwrap();
    let exp = iter(
        exp(ast::ExpKind::Tuple(vec![var_exp("x"), var_exp("y")])),
        ast::Iter::List,
        vec![var("x", vec![]), var("y", vec![])],
    );
    let ctx = ok({
        let (value_arg0, value_arg1, value_arg2) =
            (&Context::new(&global), &exp, list(&mut arena, vec![]));
        assign_exp(&mut arena, value_arg0, value_arg1, value_arg2)
    });
    for name in ["x", "y"] {
        assert!(
            get::list(&arena, &binding(&ctx, name, vec![ast::Iter::List]))
                .unwrap()
                .is_empty()
        );
    }
}
