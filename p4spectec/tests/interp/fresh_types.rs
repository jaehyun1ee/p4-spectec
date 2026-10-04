use std::rc::Rc;

use p4spectec::{
    interp::{
        shared::{
            context::{IterContext, ReadContext},
            eval::assign::assign_exp,
            prepare::ast,
            util::iterate_vars,
        },
        sl::context::{Context, Global},
    },
    lang::{
        common::{
            Iter,
            source::{FileId, Position, Span},
        },
        data::{
            typ,
            value::{Arena, Value, ValueKind, make},
        },
        il::ast as source,
        sl::ast as source_sl,
    },
    phrase,
    runtime::envs::interp::sl::ast_prepared as ast_sl,
};

/// Retains assignment and instruction-iteration variables in one loaded Global.
fn fixture(form: usize, missing: bool) -> Global {
    let mut spec = super::support::sl_spec(
        "var x : nat\nvar y : nat\ndec $fixture(nat*, nat*) : (nat, nat)*\ndef $fixture(x*, y*) = (x, y)*\n",
    );
    for def in &mut spec {
        let source_sl::DefKind::MetaFunc(source_sl::MetaFuncDef::Defined(func)) = &mut def.node
        else {
            continue;
        };
        let source_sl::InstrKind::If(instr_if) = &func.block[0].node else { unreachable!() };
        let source_sl::InstrKind::Return(instr_return) = &instr_if.block[0].node else {
            unreachable!()
        };
        let mut exp = instr_return.exp.clone();
        let source::ExpKind::Iter(exp_inner, exp_iter) = &mut exp.node else { unreachable!() };
        let source::ExpKind::Tuple(exps) = &mut exp_inner.node else { unreachable!() };
        if form == 1 {
            let exp_head = exps[0].clone();
            exps[0].node = source::ExpKind::Opt(Some(Box::new(exp_head)));
        }
        if form == 2 {
            exp_iter.iter = Iter::Opt;
        }
        let file = FileId::intern("fresh-iteration-type.watsup");
        for (idx, var) in exp_iter.vars.iter_mut().enumerate() {
            var.typ.span =
                Span::new(Position::new(file, idx + 3, 1), Position::new(file, idx + 3, 4));
        }
        exp_iter.vars.push(exp_iter.vars[0].clone());
        if missing {
            let mut var = exp_iter.vars[0].clone();
            var.id.node = Rc::from("missing");
            exp_iter.vars.push(var);
        }
        let iter_instrs = [Iter::List, Iter::Opt]
            .into_iter()
            .map(|iter| {
                let mut vars_bind = exp_iter.vars.clone();
                for var in &mut vars_bind {
                    var.iters = vec![Iter::Opt, Iter::List];
                }
                source::PremIter { iter, vars_bound: vec![], vars_bind }
            })
            .collect();
        let source_sl::ParamKind::Exp(_, exp_param) = &mut func.params[0].node else {
            unreachable!()
        };
        **exp_param = exp.clone();
        func.block.insert(
            0,
            phrase!(node: source_sl::InstrKind::Let(source_sl::LetInstr {
            exp_l: exp.clone(), exp_r: exp, iter_instrs, block: vec![],
        }), span: Span::default()),
        );
    }
    Global::load(spec).unwrap()
}

fn sentinel(arena: &mut Arena) -> Value {
    make::tuple(arena, typ::make::tuple(vec![]).node.into(), vec![], Span::default()).unwrap()
}

/// Supplies flat, nested, or optional rows without reading any output annotation.
fn input(arena: &mut Arena, form: usize, len: usize) -> Value {
    let values = (0..len)
        .map(|idx| {
            let mut value_a = make::nat(arena, (idx as u64).into(), Span::default()).unwrap();
            let value_b = make::nat(arena, (idx as u64 + 10).into(), Span::default()).unwrap();
            if form == 1 {
                value_a = make::opt(
                    arena,
                    typ::make::opt(typ::make::nat()).node.into(),
                    Some(value_a),
                    Span::default(),
                )
                .unwrap();
            }
            make::tuple(
                arena,
                typ::make::tuple(vec![]).node.into(),
                vec![value_a, value_b],
                Span::default(),
            )
            .unwrap()
        })
        .collect::<Vec<_>>();
    if form == 2 {
        make::opt(
            arena,
            typ::make::opt(typ::make::nat()).node.into(),
            values.first().copied(),
            Span::default(),
        )
        .unwrap()
    } else {
        make::list(arena, typ::make::list(typ::make::nat()).node.into(), values, Span::default())
            .unwrap()
    }
}

/// Compares every note before materialization, then leaves the arenas independent.
fn exercise(global: &Global, form: usize) -> (Arena, Arena, Vec<(Value, typ::TypKind)>) {
    let mut arena = Arena::new();
    let mut arena_reference = Arena::new();
    let mut outputs = Vec::new();
    let ctx_root = Context::new(global);
    let id = phrase!(node: Rc::from("fixture"), span: Span::default());
    let func = ctx_root.find_func(&id).unwrap();
    let ast_sl::MetaFuncDef::Defined(func_def) = &func.def else { unreachable!() };
    let ast_sl::InstrKind::Let(instr) = &func_def.block[0].node else { unreachable!() };
    let ast_sl::ParamKind::Exp(_, exp_param) = &func_def.params[0].node else { unreachable!() };
    for exp in [exp_param.as_ref(), &instr.exp_l] {
        let exp_reference = exp.clone();
        let ast::ExpKind::Iter(_, exp_iter) = &exp.node else { unreachable!() };
        let ast::ExpKind::Iter(_, iter_reference) = &exp_reference.node else { unreachable!() };
        for len in [0, if form == 2 { 1 } else { 3 }] {
            let value = input(&mut arena, form, len);
            assert_eq!(value, input(&mut arena_reference, form, len));
            for _ in 0..3 {
                let ctx = ctx_root.localize_with_layout(&func.layout);
                let vars_outer = iterate_vars(&ctx, &exp_iter.vars, exp_iter.iter);
                assert!(
                    ctx.find_iterated_type_template(&exp_iter.vars[0], exp_iter.iter)
                        .is_some()
                );
                assert!(
                    ctx.find_iterated_type_template(&iter_reference.vars[0], exp_iter.iter)
                        .is_none()
                );
                let ctx_reference =
                    assign_exp(&mut arena_reference, ctx.clone(), &exp_reference, value).unwrap();
                let ctx = assign_exp(&mut arena, ctx, exp, value).unwrap();
                for var in &vars_outer {
                    let value = *ctx.find_value_at_slot(var.slot).unwrap();
                    assert_eq!(Some(&value), ctx_reference.find_value_at_slot(var.slot));
                    outputs.push((value, var.typ().node));
                }
                assert_eq!(sentinel(&mut arena), sentinel(&mut arena_reference));
            }
        }
    }
    // These are the same output binders used by instruction list/option yields
    for iter in &instr.iter_instrs {
        let vars_reference = iter.vars_bind.clone();
        for (var, var_reference) in iter.vars_bind.iter().zip(&vars_reference) {
            assert!(
                ctx_root
                    .find_iterated_type_template(var, iter.iter)
                    .is_some()
            );
            assert!(
                ctx_root
                    .find_iterated_type_template(var_reference, iter.iter)
                    .is_none()
            );
        }
        for len in [0, 1] {
            for _ in 0..3 {
                let value = make::nat(&mut arena, 7_u64.into(), Span::default()).unwrap();
                assert_eq!(
                    value,
                    make::nat(&mut arena_reference, 7_u64.into(), Span::default()).unwrap()
                );
                let mut ctx = ctx_root.localize_with_layout(&func.layout);
                let mut ctx_reference = ctx.clone();
                let vars_outer = iterate_vars(&ctx, &iter.vars_bind, iter.iter);
                let vars_outer_reference = iterate_vars(&ctx_reference, &vars_reference, iter.iter);
                let values_by_var = vec![vec![value; len]; iter.vars_bind.len()];
                match iter.iter {
                    Iter::List => {
                        ctx.bind_list_values_by_var(&mut arena, &vars_outer, values_by_var.clone())
                            .unwrap();
                        ctx_reference
                            .bind_list_values_by_var(
                                &mut arena_reference,
                                &vars_outer_reference,
                                values_by_var,
                            )
                            .unwrap();
                    }
                    Iter::Opt => {
                        ctx.bind_opt_values_by_var(&mut arena, &vars_outer, values_by_var.clone())
                            .unwrap();
                        ctx_reference
                            .bind_opt_values_by_var(
                                &mut arena_reference,
                                &vars_outer_reference,
                                values_by_var,
                            )
                            .unwrap();
                    }
                }
                for var in &vars_outer {
                    let value = *ctx.find_value_at_slot(var.slot).unwrap();
                    assert_eq!(Some(&value), ctx_reference.find_value_at_slot(var.slot));
                    outputs.push((value, var.typ().node));
                }
                assert_eq!(sentinel(&mut arena), sentinel(&mut arena_reference));
            }
        }
    }
    (arena, arena_reference, outputs)
}

/// Reads delayed notes backwards and re-interns their exposed Rc allocations.
fn check_materialization(
    mut arena: Arena,
    mut arena_reference: Arena,
    outputs: Vec<(Value, typ::TypKind)>,
) {
    for (value, typ_expect) in outputs.into_iter().rev() {
        assert_eq!(arena.typ(&value).as_ref(), &typ_expect);
        assert_eq!(arena.typ(&value), arena_reference.typ(&value));
        let typ = arena.typ(&value).clone();
        assert!(Rc::ptr_eq(&typ, arena.typ(&value)));
        let kind: ValueKind = arena.kind(&value).map(|value| *value, Clone::clone);
        let span = *arena.span(&value);
        assert_eq!(make::new(&mut arena, kind, typ, span).unwrap(), value);
        let typ = arena_reference.typ(&value).clone();
        let kind = arena_reference
            .kind(&value)
            .map(|value| *value, Clone::clone);
        assert_eq!(make::new(&mut arena_reference, kind, typ, span).unwrap(), value);
        assert_eq!(sentinel(&mut arena), sentinel(&mut arena_reference));
    }
}

#[test]
fn fresh_iteration_notes_preserve_indices_spans_after_reset_and_global_drop() {
    for form in 0..3 {
        let global = fixture(form, false);
        let (arena, arena_reference, outputs) = exercise(&global, form);
        check_materialization(arena, arena_reference, outputs);
        // Templates survive an arena replacement and own data after Global drops
        let (arena, arena_reference, outputs) = exercise(&global, form);
        drop(global);
        check_materialization(arena, arena_reference, outputs);
    }
}

#[test]
fn missing_output_bindings_do_not_reserve_the_failed_annotation() {
    for form in 0..3 {
        let global = fixture(form, true);
        let ctx_root = Context::new(&global);
        let id = phrase!(node: Rc::from("fixture"), span: Span::default());
        let func = ctx_root.find_func(&id).unwrap();
        let ast_sl::MetaFuncDef::Defined(func_def) = &func.def else { unreachable!() };
        let ast_sl::ParamKind::Exp(_, exp) = &func_def.params[0].node else { unreachable!() };
        let exp = exp.as_ref();
        let exp_reference = exp.clone();
        let mut sentinels = Vec::new();
        for exp in [exp, &exp_reference] {
            let mut arena = Arena::new();
            let value = input(&mut arena, form, if form == 2 { 1 } else { 3 });
            let ctx = ctx_root.localize_with_layout(&func.layout);
            let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                assign_exp(&mut arena, ctx, exp, value)
            }));
            let error = match result {
                Err(error) => error,
                Ok(_) => panic!("missing output binding must fail"),
            };
            let text = error
                .downcast_ref::<String>()
                .map(String::as_str)
                .or_else(|| error.downcast_ref::<&str>().copied())
                .unwrap();
            assert_eq!(text, "value must be bound");
            sentinels.push(sentinel(&mut arena));
        }
        assert_eq!(sentinels[0], sentinels[1]);
    }
}
