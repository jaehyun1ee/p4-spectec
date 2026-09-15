use p4spec_rust::interp::al::error::{ContextErrorKind, RuntimeErrorKind};
use p4spec_rust::lang::data::value::ValueArena;

use p4spec_rust::{
    interp::al::{
        AlInterp, Config,
        backtrack::Backtrack,
        context::{Context, Global, Scope},
        error::{EntityKind, ErrorKind},
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
    runtime::typdef::TypeDef,
};

fn id(name: &str, line: i64) -> ast::Id {
    phrase!(node: name.to_owned(), span: Span::new(
        Position::new("context.watsup", line, 0),
        Position::new("context.watsup", line, 1),
    ))
}

fn func(name: &str, line: i64) -> ast::MetaFuncDef {
    ast::MetaFuncDef::Extern(ast::ExternFunc {
        id: id(name, line),
        tparams: vec![],
        params: vec![],
        typ: typ::make::bool(),
        hints: vec![],
    })
}

fn def(node: ast::DefKind) -> ast::Def {
    phrase!(node: node, span: Span::default())
}

fn var(name: &str, iters: Vec<ast::Iter>) -> ast::Var {
    ast::Var {
        id: id(name, 1),
        typ: typ::make::bool(),
        iters,
    }
}

fn variable(var: &ast::Var) -> Variable {
    Variable::new(var.id.clone(), var.iters.clone())
}

#[test]
fn test_duplicate_global_definition_uses_second_identifier_span() {
    for (kind, def_a, def_b) in [
        (
            EntityKind::Type,
            ast::DefKind::Typ(ast::TypDef::Extern(ast::ExternTyp {
                id: id("x", 1),
                hints: vec![],
            })),
            ast::DefKind::Typ(ast::TypDef::Extern(ast::ExternTyp {
                id: id("x", 9),
                hints: vec![],
            })),
        ),
        (
            EntityKind::Function,
            ast::DefKind::MetaFunc(func("x", 1)),
            ast::DefKind::MetaFunc(func("x", 9)),
        ),
    ] {
        let error = Global::load(vec![def(def_a), def(def_b)]).unwrap_err();
        assert_eq!(error.span, id("x", 9).span);
        assert_eq!(
            *error.kind,
            ErrorKind::Context(ContextErrorKind::Duplicate {
                kind,
                name: "x".into()
            })
        );
    }
}

#[test]
fn test_localize_discards_locals_and_retains_global_lookup() {
    let mut arena = ValueArena::new();
    let func_global = func("global", 1);
    let global = Global::load(vec![def(ast::DefKind::MetaFunc(func_global.clone()))]).unwrap();
    let mut ctx = Context::new(&global);
    let func_local = func("local", 2);
    ctx.add_func(id("local", 2), func_local.clone().into())
        .unwrap();
    ctx.add_typdef(id("T", 3), TypeDef::Extern).unwrap();
    let var = variable(&var("x", vec![]));
    ctx.add_value(
        var.clone(),
        make::bool(&mut arena, true, Span::default()).unwrap(),
    );
    assert_eq!(
        ctx.find_func(&id("local", 8))
            .map(|(scope, func)| (scope, func.as_ref()))
            .unwrap(),
        (Scope::Local, &func_local)
    );
    let ctx_local = ctx.localize();
    assert!(ctx_local.find_func_opt(&id("local", 8)).is_none());
    assert!(ctx_local.find_typdef_opt(&id("T", 8)).is_none());
    assert!(ctx_local.find_value_opt(&var).is_none());
    assert_eq!(
        ctx_local
            .find_func(&id("global", 8))
            .map(|(scope, func)| (scope, func.as_ref()))
            .unwrap(),
        (Scope::Global, &func_global)
    );
    assert!(ctx.find_value_opt(&var).is_some());
}

#[test]
fn test_local_definition_duplicates_do_not_replace_bindings() {
    let global = Global::load(vec![
        def(ast::DefKind::MetaFunc(func("f", 1))),
        def(ast::DefKind::Typ(ast::TypDef::Extern(ast::ExternTyp {
            id: id("T", 1),
            hints: vec![],
        }))),
    ])
    .unwrap();
    let mut ctx = Context::new(&global);
    assert!(matches!(
        *ctx.add_func(id("f", 7), func("f", 7).into())
            .unwrap_err()
            .kind,
        ErrorKind::Context(ContextErrorKind::Duplicate {
            kind: EntityKind::Function,
            ..
        })
    ));
    assert_eq!(
        ctx.add_typdef(id("T", 7), TypeDef::Parameter)
            .unwrap_err()
            .span,
        id("T", 7).span
    );
    ctx.add_typdef(id("U", 2), TypeDef::Extern).unwrap();
    ctx.add_func(id("g", 2), func("g", 2).into()).unwrap();
    assert!(ctx.add_typdef(id("U", 7), TypeDef::Parameter).is_err());
    assert!(ctx.add_func(id("g", 7), func("g", 7).into()).is_err());
    assert_eq!(ctx.find_typdef(&id("U", 8)).unwrap(), &TypeDef::Extern);
    assert_eq!(
        ctx.find_func(&id("g", 8)).unwrap().1.as_ref(),
        &func("g", 2)
    );
}

#[test]
fn test_sibling_contexts_isolate_rebinding_and_iterator_paths() {
    let mut arena = ValueArena::new();
    let global = Global::load(vec![]).unwrap();
    let mut ctx = Context::new(&global);
    let var = variable(&var("x", vec![]));
    ctx.add_value(
        var.clone(),
        make::bool(&mut arena, false, Span::default()).unwrap(),
    );
    let mut ctx_a = ctx.clone();
    let ctx_b = ctx.clone();
    ctx_a.add_value(
        Variable::new(id("x", 9), vec![]),
        make::bool(&mut arena, true, Span::default()).unwrap(),
    );
    ctx_a.add_value(
        Variable::new(id("x", 9), vec![ast::Iter::List]),
        make::bool(&mut arena, true, Span::default()).unwrap(),
    );
    assert!(get::bool(&arena, ctx_a.find_value(&var).unwrap()).unwrap());
    assert!(!get::bool(&arena, ctx_b.find_value(&var).unwrap()).unwrap());
    assert!(
        ctx.find_value_opt(&Variable::new(id("x", 1), vec![ast::Iter::List]))
            .is_none()
    );
}

#[test]
fn test_missing_value_reports_iterator_path_and_lookup_span() {
    let global = Global::load(vec![]).unwrap();
    let error = Context::new(&global)
        .find_value(&Variable::new(
            id("x", 9),
            vec![ast::Iter::List, ast::Iter::Opt],
        ))
        .unwrap_err();
    assert_eq!(error.span, id("x", 9).span);
    assert_eq!(
        *error.kind,
        ErrorKind::Context(ContextErrorKind::Undefined {
            kind: EntityKind::Value,
            name: "x*?".into()
        })
    );
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
            make::opt(
                runner.arena_mut(),
                typ.node.clone().into(),
                Some(value),
                Span::default(),
            )
            .unwrap(),
        );
    }
    let value_opt = ctx
        .map_opt(&mut runner, &span, &vars, |runner, ctx_sub| {
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
        make::opt(
            runner.arena_mut(),
            typ.node.clone().into(),
            None,
            Span::default(),
        )
        .unwrap(),
    );
    let Backtrack::Err(errors) = ctx.map_opt(&mut runner, &span, &vars, |_, _| {
        panic!("mixed optionality")
    }) else {
        panic!("expected optionality mismatch");
    };
    assert_eq!(
        *errors[0].kind,
        ErrorKind::Context(ContextErrorKind::OptionalityMismatch)
    );
    assert_eq!(errors[0].span, span);
    ctx.add_value(
        Variable::new(vars[0].id.clone(), vec![ast::Iter::List, ast::Iter::Opt]),
        make::opt(
            runner.arena_mut(),
            typ.node.clone().into(),
            None,
            Span::default(),
        )
        .unwrap(),
    );
    assert!(
        ctx.map_opt(&mut runner, &span, &vars, |_, _| panic!("absent inputs"))
            .finish()
            .unwrap()
            .is_none()
    );
    assert_eq!(
        ctx.map_opt(&mut runner, &span, &[], |_, _| Backtrack::Ok(value))
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
            make::list(
                runner.arena_mut(),
                typ::make::bool().node.into(),
                values,
                Span::default(),
            )
            .unwrap(),
        );
    }
    let mut rows = Vec::new();
    let values = ctx
        .map_list(&mut runner, &span, &vars, |runner, ctx_sub| {
            rows.push(
                vars.iter()
                    .map(|var| {
                        get::bool(runner.arena(), ctx_sub.find_value(&variable(var)).unwrap())
                            .unwrap()
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
    let result = ctx.map_list(&mut runner, &span, &vars, |_, _| {
        count += 1;
        Backtrack::Unmatch(vec![])
    });
    assert!(matches!(result, Backtrack::Unmatch(_)));
    assert_eq!(count, 1);
    ctx.add_value(
        Variable::new(vars[1].id.clone(), vec![ast::Iter::List]),
        make::list(
            runner.arena_mut(),
            typ::make::bool().node.into(),
            vec![],
            Span::default(),
        )
        .unwrap(),
    );
    let Backtrack::Err(errors) =
        ctx.map_list(&mut runner, &span, &vars, |_, _| panic!("unequal lengths"))
    else {
        panic!("expected iteration length mismatch");
    };
    assert!(matches!(
        *errors[0].kind,
        ErrorKind::Context(ContextErrorKind::IterationLengthMismatch {
            expected: 2,
            actual: 0
        })
    ));
    assert_eq!(errors[0].span, span);
    assert!(
        ctx.map_list(&mut runner, &span, &[], |_, _| panic!("no inputs"))
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
    let Backtrack::Err(errors) = ctx.map_opt(
        &mut runner,
        &id("iteration", 9).span,
        std::slice::from_ref(&var),
        |_, _| panic!("wrong input kind"),
    ) else {
        panic!("expected value kind error");
    };
    assert_eq!(errors[0].span, var.id.span);
    assert!(matches!(
        *errors[0].kind,
        ErrorKind::Runtime(RuntimeErrorKind::Value(_))
    ));
    let value = ctx
        .find_value(&Variable::new(var.id.clone(), vec![ast::Iter::Opt]))
        .unwrap();
    assert!(get::bool(runner.arena(), value).unwrap());
}

#[test]
fn test_loaded_native_spec_preserves_definition_bodies_and_locations() {
    use p4spec_rust::{
        frontend::parse::parse_files,
        pass::{algo, elaborate},
    };
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../spec");
    let spec_el = parse_files([path]).unwrap();
    let spec_il = elaborate::elaborate(spec_el).unwrap();
    let spec_al = algo::convert(spec_il).unwrap();
    let global = Global::load(spec_al.clone()).unwrap();
    let ctx = Context::new(&global);
    let ctx_clone = ctx.clone();
    let ctx_local = ctx.localize();
    for def in &spec_al {
        match &def.node {
            ast::DefKind::Typ(ast::TypDef::Defined(typdef)) => {
                let (tparams, def_typ) = ctx.find_defined_typdef(&typdef.id).unwrap();
                assert_eq!(tparams, typdef.tparams);
                assert_eq!(def_typ, &typdef.def_typ);
            }
            ast::DefKind::Typ(ast::TypDef::Extern(typdef)) => {
                assert_eq!(ctx.find_typdef(&typdef.id).unwrap(), &TypeDef::Extern);
            }
            ast::DefKind::Rel(rel) => {
                let id = match rel {
                    ast::RelDef::Extern(rel) => &rel.id,
                    ast::RelDef::Defined(rel) => &rel.id,
                };
                let rel_global = ctx.find_rel(id).unwrap();
                assert_eq!(rel_global, rel);
                assert!(std::ptr::eq(rel_global, ctx_clone.find_rel(id).unwrap()));
                assert!(std::ptr::eq(rel_global, ctx_local.find_rel(id).unwrap()));
            }
            ast::DefKind::MetaFunc(func) => {
                let id = match func {
                    ast::MetaFuncDef::Extern(func) => &func.id,
                    ast::MetaFuncDef::Builtin(func) => &func.id,
                    ast::MetaFuncDef::Table(func) => &func.id,
                    ast::MetaFuncDef::Defined(func) => &func.id,
                };
                let (scope, func_global) = ctx.find_func(id).unwrap();
                assert_eq!((scope, func_global.as_ref()), (Scope::Global, func));
                assert!(std::rc::Rc::ptr_eq(
                    func_global,
                    ctx_clone.find_func(id).unwrap().1
                ));
                assert!(std::rc::Rc::ptr_eq(
                    func_global,
                    ctx_local.find_func(id).unwrap().1
                ));
            }
            ast::DefKind::Var(var) => {
                assert!(
                    ctx.find_value_opt(&Variable::new(var.id.clone(), vec![]))
                        .is_none()
                );
            }
        }
    }
}

#[test]
fn test_duplicate_relations_share_namespace_and_report_second_span() {
    use p4spec_rust::lang::hints::input::InputHint;
    let not_typ = phrase!(node: ast::NotTypKind::Arg(typ::make::bool()), span: id("r", 1).span);
    let rel = ast::RelDef::Extern(Box::new(ast::ExternRel {
        id: id("r", 1),
        not_typ: not_typ.clone(),
        input_hint: InputHint::new(vec![0]),
        hints: vec![],
    }));
    let rel_duplicate = ast::RelDef::Defined(Box::new(ast::DefinedRel {
        id: id("r", 9),
        not_typ,
        input_hint: InputHint::new(vec![0]),
        rule_groups: vec![],
        else_group: None,
        hints: vec![],
    }));
    let error = Global::load(vec![
        def(ast::DefKind::Rel(rel)),
        def(ast::DefKind::Rel(rel_duplicate)),
    ])
    .unwrap_err();
    assert_eq!(error.span, id("r", 9).span);
    assert_eq!(
        *error.kind,
        ErrorKind::Context(ContextErrorKind::Duplicate {
            kind: EntityKind::Relation,
            name: "r".into()
        })
    );
}

#[test]
fn test_definition_lookup_errors_and_local_type_isolation() {
    let global = Global::load(vec![]).unwrap();
    let ctx = Context::new(&global);
    let id = id("missing", 8);
    for (kind, error) in [
        (EntityKind::Type, ctx.find_typdef(&id).unwrap_err()),
        (EntityKind::Relation, ctx.find_rel(&id).unwrap_err()),
        (EntityKind::Function, ctx.find_func(&id).unwrap_err()),
    ] {
        assert_eq!(error.span, id.span);
        assert_eq!(
            *error.kind,
            ErrorKind::Context(ContextErrorKind::Undefined {
                kind,
                name: id.node.clone()
            })
        );
    }
    let mut ctx_child = ctx.clone();
    ctx_child.add_typdef(id.clone(), TypeDef::Extern).unwrap();
    ctx_child
        .add_func(id.clone(), func("missing", 8).into())
        .unwrap();
    assert!(ctx.find_typdef_opt(&id).is_none());
    assert!(ctx.find_func_opt(&id).is_none());
    assert_eq!(
        *ctx_child.find_defined_typdef(&id).unwrap_err().kind,
        ErrorKind::Context(ContextErrorKind::Undefined {
            kind: EntityKind::DefinedType,
            name: id.node
        })
    );
}
