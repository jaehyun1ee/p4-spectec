//! Iterated evaluation preserves input scopes and diagnostic spans

use p4spec_rust::{
    interp::{
        al::{
            AlInterp, Config,
            context::{Context, Global},
        },
        shared::eval::iter::{map_list, map_opt},
        shared::{
            backtrack::Backtrack,
            error::{ContextErrorKind, ErrorKind, RuntimeErrorKind},
        },
    },
    lang::{
        al::ast,
        common::{
            Variable,
            source::{Position, Span},
        },
        data::{
            typ,
            value::{get, make},
        },
    },
    phrase,
    runner::{NullExtern, NullInterface, Runner},
};

fn id(name: &str, line: usize) -> ast::Id {
    phrase!(node: name.to_owned(), span: Span::new(
        Position::new("context.watsup", line, 0),
        Position::new("context.watsup", line, 1),
    ))
}

fn var(name: &str, iters: Vec<ast::Iter>) -> ast::Var {
    ast::Var { id: id(name, 1), typ: typ::make::bool(), iters }
}

fn variable(var: &ast::Var) -> Variable {
    Variable::new(var.id.clone(), var.iters.clone())
}

#[test]
fn test_map_opt_requires_agreement_and_preserves_parent() {
    let mut runner = Runner::<AlInterp, _, _>::new(
        Global::load(vec![]).unwrap(),
        AlInterp::new(Config::new(false, false, false)),
        NullInterface,
        NullExtern,
    );
    let mut runner = runner.context();
    let vars = [var("x", vec![ast::Iter::List]), var("y", vec![])];
    let typ = typ::make::bool();
    let span = id("iteration", 9).span;
    let mut ctx = Context::new(runner.spec());
    let value = make::bool(runner.arena_mut(), true, Span::default()).unwrap();
    for var in &vars {
        let mut iters = var.iters.clone();
        iters.push(ast::Iter::Opt);
        ctx.add_value(
            Variable::new(var.id.clone(), iters),
            make::opt(runner.arena_mut(), typ.node.clone().into(), Some(value), Span::default())
                .unwrap(),
        );
    }
    let value_opt = map_opt(&mut runner, &ctx, &span, &vars, |runner, ctx_sub| {
        for var in &vars {
            assert!(
                get::bool(runner.arena(), ctx_sub.find_value(&variable(var)).unwrap()).unwrap()
            );
        }
        Backtrack::Ok(value)
    })
    .finish()
    .unwrap();
    assert_eq!(value_opt, Some(value));
    for var in &vars {
        assert!(ctx.find_value_opt(&variable(var)).is_none());
    }
    ctx.add_value(
        Variable::new(vars[1].id.clone(), vec![ast::Iter::Opt]),
        make::opt(runner.arena_mut(), typ.node.clone().into(), None, Span::default()).unwrap(),
    );
    let Backtrack::Err(errors) =
        map_opt(&mut runner, &ctx, &span, &vars, |_, _| panic!("mixed optionality"))
    else {
        panic!("expected optionality mismatch");
    };
    assert_eq!(*errors[0].kind, ErrorKind::Context(ContextErrorKind::OptionalityMismatch));
    assert_eq!(errors[0].span, span);
    ctx.add_value(
        Variable::new(vars[0].id.clone(), vec![ast::Iter::List, ast::Iter::Opt]),
        make::opt(runner.arena_mut(), typ.node.clone().into(), None, Span::default()).unwrap(),
    );
    assert!(
        map_opt(&mut runner, &ctx, &span, &vars, |_, _| panic!("absent inputs"))
            .finish()
            .unwrap()
            .is_none()
    );
    assert_eq!(
        map_opt(&mut runner, &ctx, &span, &[], |_, _| { Backtrack::Ok(value) })
            .finish()
            .unwrap(),
        Some(value)
    );
}

#[test]
fn test_map_list_transposes_in_order_without_leaking_bindings() {
    let mut runner = Runner::<AlInterp, _, _>::new(
        Global::load(vec![]).unwrap(),
        AlInterp::new(Config::new(false, false, false)),
        NullInterface,
        NullExtern,
    );
    let mut runner = runner.context();
    let vars = [var("x", vec![]), var("y", vec![])];
    let span = id("iteration", 9).span;
    let mut ctx = Context::new(runner.spec());
    for (var, values) in vars.iter().zip([[true, false], [false, true]]) {
        let values = values
            .into_iter()
            .map(|b| make::bool(runner.arena_mut(), b, Span::default()).unwrap())
            .collect();
        ctx.add_value(
            Variable::new(var.id.clone(), vec![ast::Iter::List]),
            make::list(runner.arena_mut(), typ::make::bool().node.into(), values, Span::default())
                .unwrap(),
        );
    }
    let mut rows = Vec::new();
    let values = map_list(&mut runner, &ctx, &span, &vars, |runner, ctx_sub| {
        rows.push(
            vars.iter()
                .map(|var| {
                    get::bool(runner.arena(), ctx_sub.find_value(&variable(var)).unwrap()).unwrap()
                })
                .collect::<Vec<_>>(),
        );
        Backtrack::Ok(*ctx_sub.find_value(&variable(&vars[0])).unwrap())
    })
    .finish()
    .unwrap();
    assert_eq!(rows, [vec![true, false], vec![false, true]]);
    assert_eq!(
        values
            .iter()
            .map(|value| get::bool(runner.arena(), value).unwrap())
            .collect::<Vec<_>>(),
        [true, false]
    );
    assert!(ctx.find_value_opt(&variable(&vars[0])).is_none());
    let mut count = 0;
    let result = map_list(&mut runner, &ctx, &span, &vars, |_, _| {
        count += 1;
        Backtrack::Unmatch(vec![])
    });
    assert!(matches!(result, Backtrack::Unmatch(_)));
    assert_eq!(count, 1);
    ctx.add_value(
        Variable::new(vars[1].id.clone(), vec![ast::Iter::List]),
        make::list(runner.arena_mut(), typ::make::bool().node.into(), vec![], Span::default())
            .unwrap(),
    );
    let Backtrack::Err(errors) =
        map_list(&mut runner, &ctx, &span, &vars, |_, _| panic!("unequal lengths"))
    else {
        panic!("expected iteration length mismatch");
    };
    assert!(matches!(
        *errors[0].kind,
        ErrorKind::Context(ContextErrorKind::IterationLengthMismatch { expected: 2, actual: 0 })
    ));
    assert_eq!(errors[0].span, span);
    assert!(
        map_list(&mut runner, &ctx, &span, &[], |_, _| panic!("no inputs"))
            .finish()
            .unwrap()
            .is_empty()
    );
}

#[test]
fn test_iteration_rejects_wrong_value_kind_at_variable_span() {
    let mut runner = Runner::<AlInterp, _, _>::new(
        Global::load(vec![]).unwrap(),
        AlInterp::new(Config::new(false, false, false)),
        NullInterface,
        NullExtern,
    );
    let mut runner = runner.context();
    let var = var("x", vec![]);
    let mut ctx = Context::new(runner.spec());
    ctx.add_value(
        Variable::new(var.id.clone(), vec![ast::Iter::Opt]),
        make::bool(runner.arena_mut(), true, Span::default()).unwrap(),
    );
    let Backtrack::Err(errors) =
        map_opt(&mut runner, &ctx, &id("iteration", 9).span, std::slice::from_ref(&var), |_, _| {
            panic!("wrong input kind")
        })
    else {
        panic!("expected value kind error");
    };
    assert_eq!(errors[0].span, var.id.span);
    assert!(matches!(*errors[0].kind, ErrorKind::Runtime(RuntimeErrorKind::Value(_))));
    let value = ctx
        .find_value(&Variable::new(var.id.clone(), vec![ast::Iter::Opt]))
        .unwrap();
    assert!(get::bool(runner.arena(), value).unwrap());
}
