use std::{borrow::Cow, rc::Rc};

use p4spectec::{
    interp::{
        shared::context::{IterContext, ReadContext, WriteContext},
        sl::{
            context::{Context, Global},
            eval::instr::eval_instr,
            flow::Flow,
        },
    },
    lang::{
        common::{
            Iter,
            source::{FileId, Position, Span},
        },
        data::{
            notation::{Mixfix, Mixop},
            typ,
            value::{Arena, Value, ValueKind, make},
        },
        il::ast as source,
        sl::ast as sl_source,
    },
    note_phrase, phrase,
    runner::{self, Config, NullExtern},
    runtime::envs::interp::sl::ast_prepared as ast,
};

/// Loads a checked list map with additional constructor shapes for comparison.
fn fixture(deep: bool, missing: bool, empty_vars: bool) -> Global {
    let mut spec = super::support::sl_spec(
        "var x : nat\nvar y : nat\ndec $fixture(nat*, nat*) : (nat, nat)*\ndef $fixture(x*, y*) = (x, y)*\n",
    );
    for def in &mut spec {
        let sl_source::DefKind::MetaFunc(sl_source::MetaFuncDef::Defined(func)) = &mut def.node
        else {
            continue;
        };
        let sl_source::InstrKind::If(instr_if) = &mut func.block[0].node else { unreachable!() };
        let sl_source::InstrKind::Return(instr) = &mut instr_if.block[0].node else {
            unreachable!()
        };
        let source::ExpKind::Iter(exp_inner, exp_iter) = &mut instr.exp.node else {
            unreachable!()
        };
        let source::ExpKind::Tuple(exps) = &exp_inner.node else { unreachable!() };
        let exp_x = exps[0].clone();
        let exp_y = exps[1].clone();
        let mut exp_deep = exp_x.clone();
        for _ in 0..if deep { 20 } else { 1 } {
            exp_deep = note_phrase!(node: source::ExpKind::Opt(Some(Box::new(exp_deep))), note: typ::make::opt(typ::make::nat()).node, span: Span::default());
        }
        let exp_none = note_phrase!(node: source::ExpKind::Opt(None), note: typ::make::opt(typ::make::nat()).node, span: Span::default());
        let exp_struct = note_phrase!(node: source::ExpKind::Str(vec![source::ExpField {
            atom: phrase!(node: p4spectec::lang::common::notation::atom::Atom::Keyword("field".into()), span: Span::default()), exp: exp_y.clone(),
        }]), note: typ::make::nat().node, span: Span::default());
        let mixop = Rc::new(Mixop::Seq(vec![
            Mixop::Atom(
                phrase!(node: p4spectec::lang::common::notation::atom::Atom::Keyword("C".into()), span: Span::default()),
            ),
            Mixop::Arg,
            Mixop::Arg,
        ]));
        let exp_case = note_phrase!(node: source::ExpKind::Case(Box::new(Mixfix::new(mixop, vec![exp_deep, exp_struct]).unwrap())), note: typ::make::nat().node, span: Span::default());
        let exp_list = note_phrase!(node: source::ExpKind::List(vec![exp_x.clone(), exp_y, exp_x.clone()]), note: typ::make::list(typ::make::nat()).node, span: Span::default());
        let mut exps = vec![exp_case, exp_none, exp_list];
        if deep {
            // More simultaneously live operands than the inline stack can hold
            exps.extend(std::iter::repeat_n(exp_x, 20));
        }
        exps.push(note_phrase!(node: source::ExpKind::Bool(true), note: typ::make::bool().node, span: Span::default()));
        exps.push(note_phrase!(node: source::ExpKind::Num(source::Num::Nat(7_u64.into())), note: typ::make::nat().node, span: Span::default()));
        exps.push(note_phrase!(node: source::ExpKind::Text("literal".into()), note: typ::make::text().node, span: Span::default()));
        exps.push(note_phrase!(node: source::ExpKind::Id(phrase!(node: Rc::from("outer"), span: Span::default())), note: typ::make::nat().node, span: Span::default()));
        if missing {
            exps.push(note_phrase!(node: source::ExpKind::Id(phrase!(node: Rc::from("unbound"), span: Span::default())), note: typ::make::nat().node, span: Span::default()));
        }
        exp_inner.node = source::ExpKind::Tuple(exps);
        // Duplicate binders must select their final input column
        exp_iter.vars.push(exp_iter.vars[0].clone());
        if empty_vars {
            exp_iter.vars.clear();
        }
    }
    Global::load(spec).unwrap()
}

/// Resolves the fixture's existing outer-scope binding without changing its layout.
fn outer_slot(
    func: &p4spectec::runtime::envs::interp::shared::callable::Callable<ast::MetaFuncDef>,
) -> p4spectec::lang::data::var::SlotIdx {
    let mut layout = (*func.layout).clone();
    let id = layout.resolve_id(phrase!(node: Rc::from("outer"), span: Span::default()));
    assert_eq!(layout.len(), func.layout.len());
    id.slot
}

/// Includes every ordered diagnostic descendant in the comparison.
fn error_tree(error: p4spectec::runner::InterpreterError) -> String {
    fn report(value: &p4spectec::diagnostic::Report) -> String {
        format!("{:?} {:?}", value.kind, value.children.iter().map(report).collect::<Vec<_>>())
    }
    match error {
        p4spectec::runner::InterpreterError::Fatal(error) => format!("Fatal {}", report(&error)),
        p4spectec::runner::InterpreterError::Mismatch(errors) => {
            format!("Mismatch {:?}", errors.iter().map(report).collect::<Vec<_>>())
        }
    }
}

/// Preserves annotations on every visited output handle, including children.
fn handles(arena: &Arena, value: Value) -> Vec<Value> {
    let mut values = Vec::new();
    let mut pending = vec![value];
    while let Some(value) = pending.pop() {
        values.push(value);
        match arena.kind(&value) {
            ValueKind::Tuple(children) | ValueKind::List(children) => pending.extend(children),
            ValueKind::Case(value_case) => pending.extend(value_case.args()),
            ValueKind::Struct(fields) => pending.extend(fields.iter().map(|(_, value)| *value)),
            ValueKind::Opt(value) => pending.extend(value),
            _ => {}
        }
    }
    values
}

/// Builds independently annotated input columns in the same insertion order.
fn inputs(arena: &mut Arena, lens: [usize; 2]) -> [Value; 2] {
    std::array::from_fn(|col| {
        let values = (0..lens[col])
            .map(|idx| {
                let file = FileId::intern("constructor-input.p4");
                let span = Span::new(
                    Position::new(file, idx + 1, col + 1),
                    Position::new(file, idx + 1, col + 2),
                );
                make::new(
                    arena,
                    ValueKind::Num(source::Num::Nat((((idx + 1) * 10 + col) as u64).into())),
                    Rc::new(typ::make::nat().node),
                    span,
                )
                .unwrap()
            })
            .collect();
        make::list(arena, typ::make::list(typ::make::nat()).node.into(), values, Span::default())
            .unwrap()
    })
}

#[test]
fn prepared_constructor_rows_keep_all_handles_after_moves_and_resets() {
    for deep in [false, true] {
        for empty_vars in [false, true] {
            let global = fixture(deep, false, empty_vars);
            let ctx_root = Context::new(&global);
            let id = phrase!(node: Rc::from("fixture"), span: Span::default());
            let func = ctx_root.find_func(&id).unwrap();
            let ast::MetaFuncDef::Defined(func_def) = &func.def else { unreachable!() };
            let ast::InstrKind::If(instr_if) = &func_def.block[0].node else { unreachable!() };
            let instr = &instr_if.block[0];
            let ast::InstrKind::Return(instr_return) = &instr.node else { unreachable!() };
            let exp = &instr_return.exp;
            let ast::ExpKind::Iter(_, exp_iter) = &exp.node else { unreachable!() };
            let instr_reference = instr.clone();
            let ast::InstrKind::Return(instr_return_reference) = &instr_reference.node else {
                unreachable!()
            };
            let mut runner =
                runner::build_sl(vec![], Config::new(false, false, false), NullExtern).unwrap();
            let mut runner_reference =
                runner::build_sl(vec![], Config::new(false, false, false), NullExtern).unwrap();
            for len in [0, 1, 9] {
                runner.reset();
                runner_reference.reset();
                let values = inputs(runner.arena_mut(), [len, len]);
                let values_reference = inputs(runner_reference.arena_mut(), [len, len]);
                assert_eq!(values, values_reference);
                let value_outer =
                    make::nat(runner.arena_mut(), 999_u64.into(), Span::default()).unwrap();
                let value_outer_reference =
                    make::nat(runner_reference.arena_mut(), 999_u64.into(), Span::default())
                        .unwrap();
                assert_eq!(value_outer, value_outer_reference);
                let mut ctx = ctx_root.localize_with_layout(&func.layout);
                ctx.add_value_at_slot(outer_slot(func), value_outer);
                assert!(ctx.find_construct_plan(exp).is_some());
                assert!(
                    ctx.find_construct_plan(&instr_return_reference.exp)
                        .is_none()
                );
                for (idx, var) in exp_iter.vars.iter().enumerate() {
                    ctx.add_value_at_slot(ctx.find_slot_iterated(var, Iter::List), values[idx % 2]);
                    ctx.add_value_at_slot(var.slot, value_outer);
                }
                let ctx_before = ctx.clone();
                let Flow::Return(value) =
                    eval_instr(&mut runner.context(), Cow::Borrowed(&ctx), instr, false).unwrap()
                else {
                    unreachable!()
                };
                let Flow::Return(value_reference) = eval_instr(
                    &mut runner_reference.context(),
                    Cow::Borrowed(&ctx),
                    &instr_reference,
                    false,
                )
                .unwrap() else {
                    unreachable!()
                };
                let values = handles(runner.arena(), value.node);
                let values_reference = handles(runner_reference.arena(), value_reference.node);
                assert_eq!(values, values_reference);
                for (value, value_reference) in values.iter().zip(&values_reference) {
                    assert_eq!(
                        runner.arena().typ(value),
                        runner_reference.arena().typ(value_reference)
                    );
                    assert_eq!(
                        runner.arena().span(value),
                        runner_reference.arena().span(value_reference)
                    );
                }
                for var in &exp_iter.vars {
                    assert_eq!(
                        ctx.find_value_at_slot(var.slot),
                        ctx_before.find_value_at_slot(var.slot)
                    );
                }
                let sentinel = |arena: &mut Arena| {
                    make::tuple(
                        arena,
                        typ::make::tuple(vec![]).node.into(),
                        vec![],
                        Span::default(),
                    )
                    .unwrap()
                };
                assert_eq!(sentinel(runner.arena_mut()), sentinel(runner_reference.arena_mut()));
            }
        }
    }
}

#[test]
fn prepared_constructor_rows_preserve_input_errors_and_unbound_reads() {
    for missing in [false, true] {
        let global = fixture(false, missing, false);
        let ctx_root = Context::new(&global);
        let id = phrase!(node: Rc::from("fixture"), span: Span::default());
        let func = ctx_root.find_func(&id).unwrap();
        let ast::MetaFuncDef::Defined(func_def) = &func.def else { unreachable!() };
        let ast::InstrKind::If(instr_if) = &func_def.block[0].node else { unreachable!() };
        let instr = &instr_if.block[0];
        let ast::InstrKind::Return(instr_return) = &instr.node else { unreachable!() };
        let ast::ExpKind::Iter(_, exp_iter) = &instr_return.exp.node else { unreachable!() };
        for lens in [[0, 0], [2, 2], [1, 2]] {
            let mut results = Vec::new();
            let mut sentinels = Vec::new();
            for prepared in [false, true] {
                let mut runner =
                    runner::build_sl(vec![], Config::new(false, false, false), NullExtern).unwrap();
                let values = inputs(runner.arena_mut(), lens);
                let value_outer =
                    make::nat(runner.arena_mut(), 999_u64.into(), Span::default()).unwrap();
                let mut ctx = ctx_root.localize_with_layout(&func.layout);
                ctx.add_value_at_slot(outer_slot(func), value_outer);
                for (idx, var) in exp_iter.vars.iter().enumerate() {
                    ctx.add_value_at_slot(ctx.find_slot_iterated(var, Iter::List), values[idx % 2]);
                    ctx.add_value_at_slot(var.slot, value_outer);
                }
                let instr_cloned = instr.clone();
                let instr = if prepared { instr } else { &instr_cloned };
                let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    eval_instr(&mut runner.context(), Cow::Owned(ctx), instr, false)
                        .map(|flow| format!("{flow:?}"))
                        .map_err(error_tree)
                }))
                .map_err(|payload| {
                    payload
                        .downcast_ref::<String>()
                        .cloned()
                        .or_else(|| payload.downcast_ref::<&str>().map(|s| (*s).to_owned()))
                        .unwrap()
                });
                results.push(result);
                sentinels.push(
                    make::tuple(
                        runner.arena_mut(),
                        typ::make::tuple(vec![]).node.into(),
                        vec![],
                        Span::default(),
                    )
                    .unwrap(),
                );
            }
            assert_eq!(results[0], results[1], "missing {missing}, lengths {lens:?}");
            assert_eq!(sentinels[0], sentinels[1]);
        }
    }
}
