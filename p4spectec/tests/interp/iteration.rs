use std::rc::Rc;

use p4spectec::{
    interp::{
        shared::util::iterate_vars,
        sl::context::{Context, Global},
    },
    lang::{
        common::{
            Iter,
            source::{FileId, Position, Span},
        },
        data::{
            typ,
            value::{Arena, make},
            var::Var,
        },
    },
    phrase,
    runtime::envs::interp::shared::frame::FrameLayout,
};

#[test]
fn iterated_variables_preserve_slots_spans_and_fresh_type_handles() {
    let file = FileId::intern("iteration-test.watsup");
    let span = Span::new(Position::new(file, 3, 1), Position::new(file, 3, 2));
    let mut typ = typ::make::tuple(vec![typ::make::nat(), typ::make::opt(typ::make::nat())]);
    typ.span = span;
    let var = Var { id: phrase!(node: Rc::from("x"), span: span), typ, iters: vec![Iter::Opt] };
    let mut var_outer = var.clone();
    var_outer.iters.push(Iter::List);
    let typ_expect = typ::make::iterate(var_outer.typ.clone(), &var_outer.iters);
    let mut layout = FrameLayout::default();
    let var = layout.resolve_var(var);
    let var_outer = layout.resolve_var(var_outer);
    let global = Global::load(vec![]).unwrap();
    let ctx = Context::new(&global).localize_with_layout(&Rc::new(layout));
    let vars = [var];
    let vars_outer = iterate_vars(&ctx, &vars, Iter::List);
    assert_eq!(vars_outer[0].slot, var_outer.slot);
    assert_eq!(vars_outer[0].typ(), typ_expect);
    let mut arena = Arena::new();
    let value_a =
        make::list(&mut arena, vars_outer[0].typ().node.into(), vec![], Span::default()).unwrap();
    let value_b =
        make::list(&mut arena, vars_outer[0].typ().node.into(), vec![], Span::default()).unwrap();
    assert_eq!(value_a.node, value_b.node);
    assert_ne!(value_a.note, value_b.note);
    assert_eq!(arena.typ(&value_a), arena.typ(&value_b));
}
