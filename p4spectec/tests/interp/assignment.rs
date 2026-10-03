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
