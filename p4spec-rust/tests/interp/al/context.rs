use std::rc::Rc;

use p4spec_rust::{
    interp::al::{
        context::{Context, Cursor, Spec},
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
    for (kind, first, second) in [
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
        let error = Spec::load(vec![def(first), def(second)]).unwrap_err();
        assert_eq!(error.span, id("x", 9).span);
        assert_eq!(
            error.kind,
            ErrorKind::Duplicate {
                kind,
                name: "x".into()
            }
        );
    }
}

#[test]
fn test_localize_discards_locals_and_retains_global_lookup() {
    let func_global = func("global", 1);
    let spec = Spec::load(vec![def(ast::DefKind::MetaFunc(func_global.clone()))]).unwrap();
    let mut ctx = Context::new();
    let func_local = func("local", 2);
    ctx.add_func(&spec, id("local", 2), func_local.clone())
        .unwrap();
    ctx.add_typdef(&spec, id("T", 3), TypeDef::Extern).unwrap();
    let var = variable(&var("x", vec![]));
    ctx.add_value(var.clone(), make::bool(true, Span::default()));
    assert_eq!(
        ctx.find_func(&spec, &id("local", 8)).unwrap(),
        (Cursor::Local, &func_local)
    );
    let ctx_local = ctx.localize();
    assert!(ctx_local.find_func_opt(&spec, &id("local", 8)).is_none());
    assert!(ctx_local.find_typdef_opt(&spec, &id("T", 8)).is_none());
    assert!(ctx_local.find_value_opt(&var).is_none());
    assert_eq!(
        ctx_local.find_func(&spec, &id("global", 8)).unwrap(),
        (Cursor::Global, &func_global)
    );
    assert!(ctx.find_value_opt(&var).is_some());
}

#[test]
fn test_local_definition_duplicates_do_not_replace_bindings() {
    let spec = Spec::load(vec![
        def(ast::DefKind::MetaFunc(func("f", 1))),
        def(ast::DefKind::Typ(ast::TypDef::Extern(ast::ExternTyp {
            id: id("T", 1),
            hints: vec![],
        }))),
    ])
    .unwrap();
    let mut ctx = Context::new();
    assert!(matches!(
        ctx.add_func(&spec, id("f", 7), func("f", 7))
            .unwrap_err()
            .kind,
        ErrorKind::Duplicate {
            kind: EntityKind::Function,
            ..
        }
    ));
    assert_eq!(
        ctx.add_typdef(&spec, id("T", 7), TypeDef::Parameter)
            .unwrap_err()
            .span,
        id("T", 7).span
    );
    ctx.add_typdef(&spec, id("U", 2), TypeDef::Extern).unwrap();
    ctx.add_func(&spec, id("g", 2), func("g", 2)).unwrap();
    assert!(
        ctx.add_typdef(&spec, id("U", 7), TypeDef::Parameter)
            .is_err()
    );
    assert!(ctx.add_func(&spec, id("g", 7), func("g", 7)).is_err());
    assert_eq!(
        ctx.find_typdef(&spec, &id("U", 8)).unwrap(),
        &TypeDef::Extern
    );
    assert_eq!(ctx.find_func(&spec, &id("g", 8)).unwrap().1, &func("g", 2));
}

#[test]
fn test_sibling_contexts_isolate_rebinding_and_iterator_paths() {
    let mut ctx = Context::new();
    let var = variable(&var("x", vec![]));
    ctx.add_value(var.clone(), make::bool(false, Span::default()));
    let mut ctx_a = ctx.clone();
    let ctx_b = ctx.clone();
    ctx_a.add_value(
        Variable::new(id("x", 9), vec![]),
        make::bool(true, Span::default()),
    );
    ctx_a.add_value(
        Variable::new(id("x", 9), vec![ast::Iter::List]),
        make::bool(true, Span::default()),
    );
    assert!(get::bool(ctx_a.find_value(&var).unwrap()).unwrap());
    assert!(!get::bool(ctx_b.find_value(&var).unwrap()).unwrap());
    assert!(
        ctx.find_value_opt(&Variable::new(id("x", 1), vec![ast::Iter::List]))
            .is_none()
    );
}

#[test]
fn test_missing_value_reports_iterator_path_and_lookup_span() {
    let error = Context::new()
        .find_value(&Variable::new(
            id("x", 9),
            vec![ast::Iter::List, ast::Iter::Opt],
        ))
        .unwrap_err();
    assert_eq!(error.span, id("x", 9).span);
    assert_eq!(
        error.kind,
        ErrorKind::Undefined {
            kind: EntityKind::Value,
            name: "x*?".into()
        }
    );
}

#[test]
fn test_optional_subcontexts_require_agreement_and_preserve_parent() {
    let vars = [var("x", vec![ast::Iter::List]), var("y", vec![])];
    let typ = typ::make::bool();
    let mut ctx = Context::new();
    for var in &vars {
        let mut iters = var.iters.clone();
        iters.push(ast::Iter::Opt);
        ctx.add_value(
            Variable::new(var.id.clone(), iters),
            make::opt(
                &typ,
                Some(make::bool(true, Span::default())),
                Span::default(),
            ),
        );
    }
    let ctx_sub = ctx.sub_opt(&vars).unwrap().unwrap();
    for var in &vars {
        assert!(get::bool(ctx_sub.find_value(&variable(var)).unwrap()).unwrap());
        assert!(ctx.find_value_opt(&variable(var)).is_none());
    }
    ctx.add_value(
        Variable::new(vars[1].id.clone(), vec![ast::Iter::Opt]),
        make::opt(&typ, None, Span::default()),
    );
    assert_eq!(
        ctx.sub_opt(&vars).unwrap_err().kind,
        ErrorKind::OptionalityMismatch
    );
    ctx.add_value(
        Variable::new(vars[0].id.clone(), vec![ast::Iter::List, ast::Iter::Opt]),
        make::opt(&typ, None, Span::default()),
    );
    assert!(ctx.sub_opt(&vars).unwrap().is_none());
    assert!(ctx.sub_opt(&[]).unwrap().is_some());
}

#[test]
fn test_list_subcontexts_transpose_in_order_without_leaking_bindings() {
    let vars = [var("x", vec![]), var("y", vec![])];
    let mut ctx = Context::new();
    for (var, values) in vars.iter().zip([[true, false], [false, true]]) {
        let values = values
            .into_iter()
            .map(|b| make::bool(b, Span::default()))
            .collect();
        ctx.add_value(
            Variable::new(var.id.clone(), vec![ast::Iter::List]),
            make::list(&typ::make::bool(), values, Span::default()),
        );
    }
    let ctxs = ctx.sub_list(&vars).unwrap();
    let values: Vec<_> = ctxs
        .iter()
        .map(|ctx| {
            vars.iter()
                .map(|var| get::bool(ctx.find_value(&variable(var)).unwrap()).unwrap())
                .collect::<Vec<_>>()
        })
        .collect();
    assert_eq!(values, [vec![true, false], vec![false, true]]);
    assert!(ctx.find_value_opt(&variable(&vars[0])).is_none());
    ctx.add_value(
        Variable::new(vars[1].id.clone(), vec![ast::Iter::List]),
        make::list(&typ::make::bool(), vec![], Span::default()),
    );
    assert!(matches!(
        ctx.sub_list(&vars).unwrap_err().kind,
        ErrorKind::IterationLengthMismatch {
            expected: 2,
            actual: 0
        }
    ));
    assert!(ctx.sub_list(&[]).unwrap().is_empty());
}

#[test]
fn test_iteration_rejects_wrong_value_kind_at_variable_span() {
    let var = var("x", vec![]);
    let mut ctx = Context::new();
    ctx.add_value(
        Variable::new(var.id.clone(), vec![ast::Iter::Opt]),
        make::bool(true, Span::default()),
    );
    let error = ctx.sub_opt(std::slice::from_ref(&var)).unwrap_err();
    assert_eq!(error.span, var.id.span);
    assert!(matches!(error.kind, ErrorKind::Value(_)));
    let value = Rc::clone(
        ctx.find_value(&Variable::new(var.id.clone(), vec![ast::Iter::Opt]))
            .unwrap(),
    );
    assert!(get::bool(&value).unwrap());
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
    let spec = Spec::load(spec_al.clone()).unwrap();
    let ctx = Context::new();
    for def in &spec_al {
        match &def.node {
            ast::DefKind::Typ(ast::TypDef::Defined(typdef)) => {
                let (tparams, def_typ) = ctx.find_defined_typdef(&spec, &typdef.id).unwrap();
                assert_eq!(tparams, typdef.tparams);
                assert_eq!(def_typ, &typdef.def_typ);
            }
            ast::DefKind::Typ(ast::TypDef::Extern(typdef)) => {
                assert_eq!(
                    ctx.find_typdef(&spec, &typdef.id).unwrap(),
                    &TypeDef::Extern
                );
            }
            ast::DefKind::Rel(rel) => {
                let id = match rel {
                    ast::RelDef::Extern(rel) => &rel.id,
                    ast::RelDef::Defined(rel) => &rel.id,
                };
                assert_eq!(ctx.find_rel(&spec, id).unwrap(), rel);
            }
            ast::DefKind::MetaFunc(func) => {
                let id = match func {
                    ast::MetaFuncDef::Extern(func) => &func.id,
                    ast::MetaFuncDef::Builtin(func) => &func.id,
                    ast::MetaFuncDef::Table(func) => &func.id,
                    ast::MetaFuncDef::Defined(func) => &func.id,
                };
                assert_eq!(ctx.find_func(&spec, id).unwrap(), (Cursor::Global, func));
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
    let error = Spec::load(vec![
        def(ast::DefKind::Rel(rel)),
        def(ast::DefKind::Rel(rel_duplicate)),
    ])
    .unwrap_err();
    assert_eq!(error.span, id("r", 9).span);
    assert_eq!(
        error.kind,
        ErrorKind::Duplicate {
            kind: EntityKind::Relation,
            name: "r".into()
        }
    );
}

#[test]
fn test_definition_lookup_errors_and_local_type_isolation() {
    let spec = Spec::load(vec![]).unwrap();
    let ctx = Context::new();
    let id = id("missing", 8);
    for (kind, error) in [
        (EntityKind::Type, ctx.find_typdef(&spec, &id).unwrap_err()),
        (EntityKind::Relation, ctx.find_rel(&spec, &id).unwrap_err()),
        (EntityKind::Function, ctx.find_func(&spec, &id).unwrap_err()),
    ] {
        assert_eq!(error.span, id.span);
        assert_eq!(
            error.kind,
            ErrorKind::Undefined {
                kind,
                name: id.node.clone()
            }
        );
    }
    let mut ctx_child = ctx.clone();
    ctx_child
        .add_typdef(&spec, id.clone(), TypeDef::Extern)
        .unwrap();
    ctx_child
        .add_func(&spec, id.clone(), func("missing", 8))
        .unwrap();
    assert!(ctx.find_typdef_opt(&spec, &id).is_none());
    assert!(ctx.find_func_opt(&spec, &id).is_none());
    assert_eq!(
        ctx_child.find_defined_typdef(&spec, &id).unwrap_err().kind,
        ErrorKind::Undefined {
            kind: EntityKind::DefinedType,
            name: id.node
        }
    );
}
