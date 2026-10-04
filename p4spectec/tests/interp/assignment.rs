use std::rc::Rc;

use p4spectec::lang::{
    common::source::Span,
    data::{
        typ,
        value::{get, make},
    },
    traits::print::Print,
};

use super::support::sl_runner;

#[test]
fn recursive_assignment_keeps_the_callers_frame_and_last_binding() {
    use p4spectec::{
        interp::{
            shared::{context::ReadContext, eval::assign::assign_exp, prepare::ast},
            sl::context::{Context, Global},
        },
        lang::data::value::Arena,
        note_phrase, phrase,
        runtime::envs::interp::shared::frame::FrameLayout,
    };

    let mut layout = FrameLayout::default();
    let id = layout.resolve_id(phrase!(node: Rc::from("x"), span: Span::default()));
    let exp_id = note_phrase!(node: ast::ExpKind::Id(id.clone()), note: typ::make::nat().node, span: Span::default());
    let exp = note_phrase!(node: ast::ExpKind::List(vec![exp_id.clone(), exp_id]), note: typ::make::list(typ::make::nat()).node, span: Span::default());
    let global = Global::load(vec![]).unwrap();
    let ctx = Context::new(&global).localize_with_layout(&Rc::new(layout));
    let mut arena = Arena::new();
    let values =
        [11_u64, 22].map(|num| make::nat(&mut arena, num.into(), Span::default()).unwrap());
    let value = make::list(
        &mut arena,
        typ::make::list(typ::make::nat()).node.into(),
        values.to_vec(),
        Span::default(),
    )
    .unwrap();

    let ctx_result = assign_exp(&mut arena, ctx.clone(), &exp, value).unwrap();
    assert_eq!(ctx.find_value_at_slot(id.slot), None);
    assert_eq!(ctx_result.find_value_at_slot(id.slot), Some(&values[1]));
}

#[test]
fn composite_assignment_keeps_children_when_recursive_assignment_grows_the_arena() {
    use p4spectec::{
        interp::{
            shared::{context::ReadContext, eval::assign::assign_exp, prepare::ast},
            sl::context::{Context, Global},
        },
        lang::data::value::Arena,
        note_phrase, phrase,
        runtime::envs::interp::shared::frame::FrameLayout,
    };

    let mut layout = FrameLayout::default();
    let ids = ["head", "tail", "last"]
        .map(|name| layout.resolve_id(phrase!(node: Rc::from(name), span: Span::default())));
    let exps = ids.clone().map(|id| {
        note_phrase!(node: ast::ExpKind::Id(id), note: typ::make::nat().node, span: Span::default())
    });
    let exp_cons = note_phrase!(
        node: ast::ExpKind::Cons(Box::new(exps[0].clone()), Box::new(exps[1].clone())),
        note: typ::make::list(typ::make::nat()).node, span: Span::default()
    );
    let exp = note_phrase!(
        node: ast::ExpKind::Tuple(vec![exp_cons, exps[2].clone()]),
        note: typ::make::nat().node, span: Span::default()
    );
    let global = Global::load(vec![]).unwrap();
    let ctx = Context::new(&global).localize_with_layout(&Rc::new(layout));
    let mut arena = Arena::new();
    let values =
        [11_u64, 22, 33].map(|num| make::nat(&mut arena, num.into(), Span::default()).unwrap());
    let value_list = make::list(
        &mut arena,
        typ::make::list(typ::make::nat()).node.into(),
        values[..2].to_vec(),
        Span::default(),
    )
    .unwrap();
    let value_tuple = make::tuple(
        &mut arena,
        typ::make::nat().node.into(),
        vec![value_list, values[2]],
        Span::default(),
    )
    .unwrap();

    let ctx = assign_exp(&mut arena, ctx, &exp, value_tuple).unwrap();
    assert_eq!(ctx.find_value_at_slot(ids[0].slot), Some(&values[0]));
    assert_eq!(ctx.find_value_at_slot(ids[2].slot), Some(&values[2]));
    let value_tail = ctx.find_value_at_slot(ids[1].slot).unwrap();
    assert_eq!(get::list(&arena, value_tail).unwrap(), &values[1..2]);
    assert_eq!(get::tuple(&arena, &value_tuple).unwrap(), &[value_list, values[2]]);
}

#[test]
fn list_patterns_keep_optional_rows_and_column_order() {
    let source = r#"
var x : nat
var y : nat
dec $columns((nat, nat?)*) : (nat*, nat?*)
def $columns((x, y?)*) = (x*, y?*)
"#;
    let mut runner = sl_runner(source);
    let typ_opt: Rc<_> = typ::make::opt(typ::make::nat()).node.into();
    let typ_row = typ::make::tuple(vec![typ::make::nat(), typ::make::opt(typ::make::nat())]);
    let typ_rows: Rc<_> = typ::make::list(typ_row.clone()).node.into();
    let typ_row: Rc<_> = typ_row.node.into();
    let mut values = Vec::new();
    for (num, num_opt) in [(11_u64, Some(12)), (21, None), (31, Some(32)), (41, None)] {
        let arena = runner.arena_mut();
        let value = make::nat(arena, num.into(), Span::default()).unwrap();
        let value_opt =
            num_opt.map(|num: u64| make::nat(arena, num.into(), Span::default()).unwrap());
        let value_opt = make::opt(arena, typ_opt.clone(), value_opt, Span::default()).unwrap();
        values.push(
            make::tuple(arena, typ_row.clone(), vec![value, value_opt], Span::default()).unwrap(),
        );
    }
    let value_rows =
        make::list(runner.arena_mut(), typ_rows.clone(), values, Span::default()).unwrap();
    let value = runner
        .context()
        .call_func("columns", &[], &[value_rows])
        .unwrap();
    let arena = runner.arena();
    let values = get::tuple(arena, &value).unwrap();
    let nums = get::list(arena, &values[0])
        .unwrap()
        .iter()
        .map(|value| get::num(arena, value).unwrap().to_string())
        .collect::<Vec<_>>();
    let nums_opt = get::list(arena, &values[1])
        .unwrap()
        .iter()
        .map(|value| {
            get::opt(arena, value)
                .unwrap()
                .map(|value| get::num(arena, &value).unwrap().to_string())
        })
        .collect::<Vec<_>>();
    assert_eq!(nums, ["11", "21", "31", "41"]);
    assert_eq!(nums_opt, [Some("12".into()), None, Some("32".into()), None]);

    let value_rows = make::list(runner.arena_mut(), typ_rows, vec![], Span::default()).unwrap();
    let value = runner
        .context()
        .call_func("columns", &[], &[value_rows])
        .unwrap();
    let arena = runner.arena();
    for value in get::tuple(arena, &value).unwrap() {
        assert!(get::list(arena, value).unwrap().is_empty());
    }
}

#[test]
fn mixed_parameters_keep_last_values_and_late_function_binding_errors() {
    use p4spectec::{
        diagnostic::Report,
        interp::{
            shared::{
                backtrack::WithFrame,
                context::{ReadContext, WriteContext},
                eval::assign::{assign_def, assign_exp},
            },
            sl::{
                context::{Context, Global},
                eval::call::invoke_func,
            },
        },
        lang::{
            common::source::{FileId, Position},
            sl::ast as source,
        },
        phrase,
        runner::{self, Config, InterpreterError, NullExtern},
        runtime::envs::interp::sl::ast_prepared as ast,
    };

    fn report_tree(report: &Report) -> String {
        format!(
            "{:?} {:?}",
            report.kind,
            report.children.iter().map(report_tree).collect::<Vec<_>>()
        )
    }
    fn error_tree(error: &InterpreterError) -> String {
        match error {
            InterpreterError::Fatal(report) => format!("Fatal {}", report_tree(report)),
            InterpreterError::Mismatch(reports) => {
                format!("Mismatch {:?}", reports.iter().map(report_tree).collect::<Vec<_>>())
            }
        }
    }
    let span = Span::new(
        Position::new(FileId::intern("mixed-params.watsup"), 8, 2),
        Position::new(FileId::intern("mixed-params.watsup"), 8, 9),
    );
    for failure in [None, Some("missing"), Some("duplicate")] {
        let mut spec = super::support::sl_spec(
            "var n : nat\nextern dec $identity(nat) : nat\ndec $mixed(nat, nat, nat, nat, nat) : nat\ndef $mixed(n_a, n_b, n_c, n_d, n_e) = n_c\n",
        );
        for def in &mut spec {
            let source::DefKind::MetaFunc(source::MetaFuncDef::Defined(func)) = &mut def.node
            else {
                continue;
            };
            func.params[0] = func.params[2].clone();
            for (idx, name) in
                [(1, "alias"), (3, if failure == Some("duplicate") { "alias" } else { "second" })]
            {
                func.params[idx] = phrase!(node: source::ParamKind::Def(phrase!(node: Rc::from(name), span: span), vec![], vec![], typ::make::nat()), span: span);
            }
            if failure.is_some() {
                let source::ParamKind::Exp(_, exp) = &mut func.params[4].node else {
                    unreachable!()
                };
                // A later invalid binder must never run after the function binding fails
                exp.node = source::ExpKind::Num(source::Num::Nat(99_u64.into()));
            }
        }
        let global = Global::load(spec).unwrap();
        let ctx_root = Context::new(&global);
        let id = phrase!(node: Rc::from("mixed"), span: span);
        let func = ctx_root.find_func(&id).unwrap();
        let ast::MetaFuncDef::Defined(func_def) = &func.def else { unreachable!() };
        let ast::ParamKind::Exp(_, exp) = &func_def.params[2].node else { unreachable!() };
        let ast::ExpKind::Id(id_value) = &exp.node else { unreachable!() };
        let mut runner =
            runner::build_sl(vec![], Config::new(false, false, false), NullExtern).unwrap();
        let values = [11_u64, 22, 77]
            .map(|num| make::nat(runner.arena_mut(), num.into(), Span::default()).unwrap());
        let value_func = make::func(
            runner.arena_mut(),
            phrase!(node: Rc::from("identity"), span: span),
            vec![],
            vec![typ::make::nat()],
            typ::make::nat(),
            span,
        )
        .unwrap();
        let value_last = if failure == Some("missing") {
            make::func(
                runner.arena_mut(),
                phrase!(node: Rc::from("missing"), span: span),
                vec![],
                vec![typ::make::nat()],
                typ::make::nat(),
                span,
            )
            .unwrap()
        } else {
            value_func
        };
        let mut ctx = ctx_root.localize_with_layout(&func.layout);
        ctx.add_value_at_slot(id_value.slot, values[2]);
        let ctx_before = ctx.clone();
        let values_args = [values[0], value_func, values[1], value_last, values[2]];
        let result = invoke_func(&mut runner.context(), &ctx, &id, &[], &values_args);
        assert_eq!(ctx.find_value_at_slot(id_value.slot), Some(&values[2]));
        assert_eq!(ctx_before.find_value_at_slot(id_value.slot), Some(&values[2]));
        assert!(
            ctx.find_func(&phrase!(node: Rc::from("alias"), span: span))
                .is_err()
        );
        if failure.is_none() {
            assert_eq!(result.unwrap(), values[1]);
        } else {
            let mut ctx_reference = ctx.localize_with_layout(&func.layout);
            for (param, value) in func_def.params[..3].iter().zip(&values_args) {
                ctx_reference = match &param.node {
                    ast::ParamKind::Exp(_, exp) => {
                        assign_exp(runner.arena_mut(), ctx_reference, exp, *value).unwrap()
                    }
                    ast::ParamKind::Def(id, ..) => {
                        assign_def(runner.arena(), &ctx, ctx_reference, id, *value).unwrap()
                    }
                };
            }
            let ast::ParamKind::Def(id_param, ..) = &func_def.params[3].node else {
                unreachable!()
            };
            let error_reference =
                assign_def(runner.arena(), &ctx, ctx_reference, id_param, value_last)
                    .with_frame(span, || {
                        p4spectec::interp::shared::error::trace::message_func_invocation(&id, &[])
                    })
                    .err()
                    .unwrap();
            assert_eq!(error_tree(&result.unwrap_err()), error_tree(&error_reference));
        }
    }
}
