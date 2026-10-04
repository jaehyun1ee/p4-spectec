use std::{borrow::Cow, rc::Rc};

use p4spectec::{
    diagnostic::{Report, ReportKind},
    interp::{
        shared::context::ReadContext,
        shared::context::WriteContext,
        sl::{
            context::{Context, Global},
            eval::instr::eval_instr,
            flow::Flow,
        },
    },
    lang::{
        common::{
            prim::num::CmpOp,
            source::{FileId, Position, Span},
        },
        data::{
            typ,
            value::{Arena, ValueKind, make},
        },
        il::ast as source,
        sl::ast as sl_source,
    },
    note_phrase, phrase,
    runner::{self, Config, NullExtern},
    runtime::envs::interp::sl::ast_prepared as ast,
};

/// Keeps the public context's unwind auto-traits available to callers.
#[test]
fn loaded_contexts_remain_ref_unwind_safe() {
    fn check<T: std::panic::RefUnwindSafe>() {}
    check::<Global>();
    check::<Context<'static>>();
}

/// Adds a condition whose truth changes with its current argument.
fn fixture(case: bool) -> Global {
    let mut spec =
        super::support::sl_spec("var n : nat\ndec $fixture(nat) : nat\ndef $fixture(n) = n\n");
    for def in &mut spec {
        let sl_source::DefKind::MetaFunc(sl_source::MetaFuncDef::Defined(func)) = &mut def.node
        else {
            continue;
        };
        let file = FileId::intern("condition-cache.watsup");
        let span = Span::new(Position::new(file, 3, 2), Position::new(file, 3, 8));
        let exp_l = note_phrase!(node: source::ExpKind::Id(phrase!(node: Rc::from("n"), span: span)), note: typ::make::nat().node, span: span);
        let exp_r = note_phrase!(node: source::ExpKind::Num(source::Num::Nat(0_u64.into())), note: typ::make::nat().node, span: span);
        let exp = note_phrase!(node: source::ExpKind::Cmp(source::CmpOp::Num(CmpOp::Gt), source::OpTyp::Nat, Box::new(exp_l), Box::new(exp_r)), note: typ::make::bool().node, span: span);
        let block = std::mem::take(&mut func.block);
        let instr = if case {
            sl_source::InstrKind::Case(sl_source::CaseInstr {
                exp,
                cases: vec![sl_source::Case { guard: sl_source::Guard::Bool(true), block }],
                dangle: false,
            })
        } else {
            sl_source::InstrKind::If(sl_source::IfInstr {
                exp,
                iter_exps: vec![],
                block,
                dangle: false,
            })
        };
        func.block.push(phrase!(node: instr, span: span));
    }
    Global::load(spec).unwrap()
}

/// Includes all diagnostic fields and descendants, then mutates returned messages.
fn outcome(flow: Flow, case: bool) -> String {
    fn report_tree(report: &Report) -> String {
        format!(
            "{:?} {:?}",
            report.kind,
            report.children.iter().map(report_tree).collect::<Vec<_>>()
        )
    }
    match flow {
        Flow::Cont(mut reports) => {
            let text = format!("{:?}", reports.iter().map(report_tree).collect::<Vec<_>>());
            let ReportKind::Cause(diagnostic) = &mut reports[0].kind else { unreachable!() };
            assert!(diagnostic.message.starts_with(if case {
                "condition case "
            } else {
                "condition "
            }));
            diagnostic.message.clear();
            diagnostic.message.push_str("mutated returned report");
            diagnostic.code = Some("mutated code".into());
            diagnostic.labels.clear();
            diagnostic.notes.push("mutated note".into());
            text
        }
        Flow::Return(value) => format!("Return {value:?}"),
        _ => unreachable!(),
    }
}

/// Compares cached failures with cloned syntax through resets and changed inputs.
fn check(global: Global, case: bool) -> Global {
    let ctx_root = Context::new(&global);
    let id = phrase!(node: Rc::from("fixture"), span: Span::default());
    let func = ctx_root.find_func(&id).unwrap();
    let ast::MetaFuncDef::Defined(func_def) = &func.def else { unreachable!() };
    let instr = &func_def.block[0];
    let instr_reference = instr.clone();
    let mut layout = (*func.layout).clone();
    let id = layout.resolve_id(phrase!(node: Rc::from("n"), span: Span::default()));
    assert_eq!(layout.len(), func.layout.len());
    let mut runner =
        runner::build_sl(vec![], Config::new(false, false, false), NullExtern).unwrap();
    let mut runner_reference =
        runner::build_sl(vec![], Config::new(false, false, false), NullExtern).unwrap();
    for (idx, num) in [0_u64, 0, 2, 0, 0].into_iter().enumerate() {
        if idx == 3 {
            runner.reset();
            runner_reference.reset();
        }
        let mut outputs = Vec::new();
        for (runner, instr) in [(&mut runner, instr), (&mut runner_reference, &instr_reference)] {
            let value = make::nat(runner.arena_mut(), num.into(), Span::default()).unwrap();
            let mut ctx = ctx_root.localize_with_layout(&func.layout);
            ctx.add_value_at_slot(id.slot, value);
            let flow = eval_instr(&mut runner.context(), Cow::Owned(ctx), instr, false).unwrap();
            assert_eq!(matches!(flow, Flow::Return(_)), num > 0);
            outputs.push(outcome(flow, case));
        }
        assert_eq!(outputs[0], outputs[1], "case {case}, run {idx}");
        let sentinel = |arena: &mut Arena| {
            make::new(
                arena,
                ValueKind::Bool(false),
                Rc::new(typ::make::bool().node),
                Span::default(),
            )
            .unwrap()
        };
        assert_eq!(sentinel(runner.arena_mut()), sentinel(runner_reference.arena_mut()));
    }
    global
}

#[test]
fn repeated_condition_failures_preserve_complete_diagnostics_after_moves_and_resets() {
    for case in [false, true] {
        let global = check(fixture(case), case);
        let globals = vec![global];
        let global = globals.into_iter().next().unwrap();
        check(global, case);
    }
}

/// Includes every ordered report descendant and the terminal flow payload.
fn flow_record(flow: &Flow) -> String {
    fn report_tree(report: &Report) -> String {
        format!(
            "{:?} {:?}",
            report.kind,
            report.children.iter().map(report_tree).collect::<Vec<_>>()
        )
    }
    match flow {
        Flow::Cont(reports) => {
            format!("Cont {:?}", reports.iter().map(report_tree).collect::<Vec<_>>())
        }
        flow => format!("{flow:?}"),
    }
}

#[test]
fn singleton_blocks_preserve_conditions_owned_contexts_and_failure_order() {
    use p4spectec::interp::sl::eval::instr::eval_block;
    for case in [false, true] {
        let global = fixture(case);
        let ctx_root = Context::new(&global);
        let id = phrase!(node: Rc::from("fixture"), span: Span::default());
        let func = ctx_root.find_func(&id).unwrap();
        let ast::MetaFuncDef::Defined(func_def) = &func.def else { unreachable!() };
        let instr = &func_def.block[0];
        let mut layout = (*func.layout).clone();
        let id = layout.resolve_id(phrase!(node: Rc::from("n"), span: Span::default()));
        assert_eq!(layout.len(), func.layout.len());
        for det in [false, true] {
            for borrowed in [false, true] {
                for tail in [false, true] {
                    for num in [0_u64, 2] {
                        let mut records = Vec::new();
                        for block in [false, true] {
                            let mut runner = runner::build_sl(
                                vec![],
                                Config::new(false, det, false),
                                NullExtern,
                            )
                            .unwrap();
                            let value =
                                make::nat(runner.arena_mut(), num.into(), Span::default()).unwrap();
                            let mut ctx = ctx_root.localize_with_layout(&func.layout);
                            ctx.add_value_at_slot(id.slot, value);
                            let ctx_borrow = ctx.clone();
                            let ctx =
                                if borrowed { Cow::Borrowed(&ctx_borrow) } else { Cow::Owned(ctx) };
                            let flow = if block {
                                eval_block(
                                    &mut runner.context(),
                                    ctx,
                                    std::slice::from_ref(instr),
                                    tail,
                                )
                            } else {
                                eval_instr(&mut runner.context(), ctx, instr, tail)
                            }
                            .unwrap();
                            let value_sentinel = make::new(
                                runner.arena_mut(),
                                ValueKind::Bool(false),
                                Rc::new(typ::make::bool().node),
                                Span::default(),
                            )
                            .unwrap();
                            records.push((flow_record(&flow), value_sentinel));
                        }
                        assert_eq!(
                            records[0], records[1],
                            "case {case}, det {det}, borrowed {borrowed}, tail {tail}, input {num}"
                        );
                    }
                }
            }
            let mut runner =
                runner::build_sl(vec![], Config::new(false, det, false), NullExtern).unwrap();
            let value = make::nat(runner.arena_mut(), 0_u64.into(), Span::default()).unwrap();
            let mut ctx = ctx_root.localize_with_layout(&func.layout);
            ctx.add_value_at_slot(id.slot, value);
            let Flow::Cont(reports) =
                eval_block(&mut runner.context(), Cow::Borrowed(&ctx), &[], true).unwrap()
            else {
                unreachable!()
            };
            assert!(reports.is_empty());
            let block = [instr.clone(), instr.clone()];
            let Flow::Cont(reports) =
                eval_block(&mut runner.context(), Cow::Borrowed(&ctx), &block, true).unwrap()
            else {
                unreachable!()
            };
            assert_eq!(reports.len(), if det { 2 } else { 1 });
        }
    }
}

#[test]
fn singleton_blocks_preserve_call_tail_position() {
    use p4spectec::interp::sl::eval::instr::eval_block;
    let global = Global::load(super::support::sl_spec("var n : nat\ndec $identity(nat) : nat\ndef $identity(n) = n\ndec $fixture(nat) : nat\ndef $fixture(n) = $identity(n)\n")).unwrap();
    let ctx_root = Context::new(&global);
    let id = phrase!(node: Rc::from("fixture"), span: Span::default());
    let func = ctx_root.find_func(&id).unwrap();
    let ast::MetaFuncDef::Defined(func_def) = &func.def else { unreachable!() };
    let instr = &func_def.block[0];
    assert!(matches!(instr.node, ast::InstrKind::Return(_)));
    let mut layout = (*func.layout).clone();
    let id = layout.resolve_id(phrase!(node: Rc::from("n"), span: Span::default()));
    assert_eq!(layout.len(), func.layout.len());
    for tail in [false, true] {
        let mut records = Vec::new();
        for block in [false, true] {
            let mut runner =
                runner::build_sl(vec![], Config::new(false, false, false), NullExtern).unwrap();
            let value = make::nat(runner.arena_mut(), 7_u64.into(), Span::default()).unwrap();
            let mut ctx = ctx_root.localize_with_layout(&func.layout);
            ctx.add_value_at_slot(id.slot, value);
            let flow = if block {
                eval_block(
                    &mut runner.context(),
                    Cow::Owned(ctx),
                    std::slice::from_ref(instr),
                    tail,
                )
            } else {
                eval_instr(&mut runner.context(), Cow::Owned(ctx), instr, tail)
            }
            .unwrap();
            assert_eq!(matches!(flow, Flow::TailFunc(_)), tail);
            let value_sentinel = make::new(
                runner.arena_mut(),
                ValueKind::Bool(false),
                Rc::new(typ::make::bool().node),
                Span::default(),
            )
            .unwrap();
            records.push((flow_record(&flow), value_sentinel));
        }
        assert_eq!(records[0], records[1]);
    }
}

#[test]
fn singleton_mismatch_keeps_sequential_and_deterministic_distinction() {
    use p4spectec::interp::sl::eval::instr::eval_block;
    use p4spectec::runner::InterpreterError;
    let mut spec = super::support::sl_spec(
        "dec $failure() : bool\ndef $failure() = false\ndec $fixture() : bool\ndef $fixture() = $failure()\n",
    );
    for def in &mut spec {
        let sl_source::DefKind::MetaFunc(sl_source::MetaFuncDef::Defined(func)) = &mut def.node
        else {
            continue;
        };
        if func.id.node.as_ref() == "failure" {
            // An exhausted function supplies a recoverable mismatch to its caller
            func.block.clear();
            func.block_else = None;
        } else if func.id.node.as_ref() == "fixture" {
            let sl_source::InstrKind::Return(instr_return) = &func.block[0].node else {
                unreachable!()
            };
            let exp = instr_return.exp.clone();
            func.block = vec![
                phrase!(node: sl_source::InstrKind::If(sl_source::IfInstr { exp, iter_exps:vec![],block:vec![],dangle:false }), span: Span::default()),
            ];
        }
    }
    let global = Global::load(spec).unwrap();
    let ctx_root = Context::new(&global);
    let id = phrase!(node: Rc::from("fixture"), span: Span::default());
    let func = ctx_root.find_func(&id).unwrap();
    let ast::MetaFuncDef::Defined(func_def) = &func.def else { unreachable!() };
    let instr = &func_def.block[0];
    for det in [false, true] {
        let mut runner =
            runner::build_sl(vec![], Config::new(false, det, false), NullExtern).unwrap();
        let ctx = ctx_root.localize_with_layout(&func.layout);
        assert!(matches!(
            eval_instr(&mut runner.context(), Cow::Borrowed(&ctx), instr, false),
            Err(InterpreterError::Mismatch(_))
        ));
        let result =
            eval_block(&mut runner.context(), Cow::Owned(ctx), std::slice::from_ref(instr), false);
        if det {
            assert!(matches!(result,Ok(Flow::Cont(reports)) if reports.is_empty()));
        } else {
            assert!(matches!(result, Err(InterpreterError::Mismatch(_))));
        }
    }
}

#[derive(Clone, Copy, Debug)]
enum PendingScenario {
    Leaves,
    Deep,
    Return,
    Nondeterminism,
}

/// Gives each failed leaf and conclusion a distinct source location.
fn pending_span(line: usize) -> Span {
    let file = FileId::intern("pending-condition.watsup");
    Span::new(Position::new(file, line, 1), Position::new(file, line, 9))
}

/// Constructs a located boolean condition around an arbitrary nested block.
fn pending_if(cond: bool, line: usize, block: sl_source::Block) -> sl_source::Instr {
    let span = pending_span(line);
    let exp =
        note_phrase!(node: source::ExpKind::Bool(cond), note: typ::make::bool().node, span: span);
    phrase!(node: sl_source::InstrKind::If(sl_source::IfInstr {
        exp, iter_exps: vec![], block, dangle: false,
    }), span: span)
}

/// Registers nested leaves, deeper called failures, and explicit conclusions.
fn pending_spec(scenario: PendingScenario, fallback: Option<bool>, table: bool) -> sl_source::Spec {
    // Keep the failing call below arithmetic, including under deterministic tails
    let mut spec = super::support::sl_spec(
        "var n : nat\ndec $failure(nat) : nat\ndef $failure(n) = n\ndec $fixture(nat) : nat\ndef $fixture(n) = $($failure(n) + 0)\n",
    );
    let mut instr_return = None;
    for def in &mut spec {
        let sl_source::DefKind::MetaFunc(sl_source::MetaFuncDef::Defined(func)) = &mut def.node
        else {
            continue;
        };
        if func.id.node.as_ref() == "failure" {
            instr_return = Some(func.block[0].clone());
            func.block = vec![pending_if(false, 30, vec![])];
            func.block_else = None;
        }
    }
    for def in &mut spec {
        let sl_source::DefKind::MetaFunc(sl_source::MetaFuncDef::Defined(func)) = &mut def.node
        else {
            continue;
        };
        if func.id.node.as_ref() != "fixture" {
            continue;
        }
        let instr_call = func.block[0].clone();
        let mut instr_return = instr_return.clone().unwrap();
        instr_return.span = pending_span(40);
        let span = pending_span(20);
        let exp = note_phrase!(node: source::ExpKind::Bool(false), note: typ::make::bool().node, span: span);
        let instr_case = phrase!(node: sl_source::InstrKind::Case(sl_source::CaseInstr {
            exp, cases: vec![sl_source::Case { guard: sl_source::Guard::Bool(true), block: vec![] }],
            dangle: false,
        }), span: span);
        // Empty branches surround a registered If leaf and a nested Case leaf
        let mut block = vec![pending_if(true, 1, vec![]), pending_if(false, 10, vec![])];
        if matches!(scenario, PendingScenario::Deep) {
            block.push(instr_call);
        }
        block.push(pending_if(true, 19, vec![instr_case]));
        block.push(pending_if(true, 21, vec![]));
        if matches!(scenario, PendingScenario::Return | PendingScenario::Nondeterminism) {
            block.push(instr_return.clone());
        }
        if matches!(scenario, PendingScenario::Nondeterminism) {
            let mut instr = instr_return.clone();
            instr.span = pending_span(41);
            block.push(instr);
        }
        func.block = block;
        func.block_else = if table {
            None
        } else {
            fallback.map(|returns| if returns { vec![instr_return.clone()] } else { vec![] })
        };
        if table {
            let sl_source::InstrKind::Return(instr) = &instr_return.node else { unreachable!() };
            let exp = instr.exp.clone();
            let mut table_rows = vec![sl_source::TableRow {
                exps_input: vec![],
                exp: exp.clone(),
                block: func.block.clone(),
            }];
            if let Some(returns) = fallback {
                table_rows.push(sl_source::TableRow {
                    exps_input: vec![],
                    exp,
                    block: if returns { vec![instr_return] } else { vec![] },
                });
            }
            let func_table = sl_source::TableFunc {
                id: func.id.clone(),
                params: func.params.clone(),
                typ: func.typ.clone(),
                table_rows,
                hints: func.hints.clone(),
            };
            def.node = sl_source::DefKind::MetaFunc(sl_source::MetaFuncDef::Table(func_table));
        }
    }
    spec
}

/// Includes complete public failures as well as successful flow payloads.
fn pending_record(result: &Result<Flow, runner::InterpreterError>) -> String {
    fn report_tree(report: &Report) -> String {
        format!(
            "{:?} {:?}",
            report.kind,
            report.children.iter().map(report_tree).collect::<Vec<_>>()
        )
    }
    match result {
        Ok(flow) => flow_record(flow),
        Err(runner::InterpreterError::Mismatch(reports)) => {
            format!("Mismatch {:?}", reports.iter().map(report_tree).collect::<Vec<_>>())
        }
        Err(runner::InterpreterError::Fatal(report)) => format!("Fatal {}", report_tree(report)),
    }
}

#[test]
fn pending_conditions_match_eager_syntax_for_selection_depth_and_order() {
    use p4spectec::interp::sl::eval::instr::eval_block;
    for scenario in [
        PendingScenario::Leaves,
        PendingScenario::Deep,
        PendingScenario::Return,
        PendingScenario::Nondeterminism,
    ] {
        let global = Global::load(pending_spec(scenario, None, false)).unwrap();
        let ctx_root = Context::new(&global);
        let id = phrase!(node: Rc::from("fixture"), span: Span::default());
        let func = ctx_root.find_func(&id).unwrap();
        let ast::MetaFuncDef::Defined(func_def) = &func.def else { unreachable!() };
        let block_reference = func_def.block.clone();
        let mut layout = (*func.layout).clone();
        let id_param = layout.resolve_id(phrase!(node: Rc::from("n"), span: Span::default()));
        assert_eq!(layout.len(), func.layout.len());
        for det in [false, true] {
            for cache in [false, true] {
                let mut runner =
                    runner::build_sl(vec![], Config::new(cache, det, false), NullExtern).unwrap();
                let mut runner_reference =
                    runner::build_sl(vec![], Config::new(cache, det, false), NullExtern).unwrap();
                for repeat in 0..3 {
                    if repeat == 2 {
                        runner.reset();
                        runner_reference.reset();
                    }
                    let mut records = Vec::new();
                    for (runner, block) in [
                        (&mut runner, func_def.block.as_slice()),
                        (&mut runner_reference, block_reference.as_slice()),
                    ] {
                        let value =
                            make::nat(runner.arena_mut(), 7_u64.into(), Span::default()).unwrap();
                        let mut ctx = ctx_root.localize_with_layout(&func.layout);
                        ctx.add_value_at_slot(id_param.slot, value);
                        let result =
                            eval_block(&mut runner.context(), Cow::Owned(ctx), block, false);
                        match scenario {
                            PendingScenario::Leaves => {
                                let Ok(Flow::Cont(reports)) = &result else {
                                    panic!("expected leaves")
                                };
                                assert_eq!(reports.len(), if det { 2 } else { 1 });
                                let ReportKind::Cause(diagnostic) = &reports.last().unwrap().kind
                                else {
                                    unreachable!()
                                };
                                assert_eq!(diagnostic.labels[0].span, pending_span(20));
                            }
                            PendingScenario::Deep => {
                                let Ok(Flow::Cont(reports)) = &result else {
                                    panic!("expected deep failure")
                                };
                                assert_eq!(reports.len(), if det { 3 } else { 1 });
                                assert!(reports.iter().any(|report| report.depth_max() > 1));
                            }
                            PendingScenario::Return => {
                                assert!(
                                    matches!(result, Ok(Flow::Return(ref value_ret)) if value_ret.node == value)
                                );
                            }
                            PendingScenario::Nondeterminism => {
                                assert!(if det {
                                    matches!(result, Err(runner::InterpreterError::Fatal(_)))
                                } else {
                                    matches!(result, Ok(Flow::Return(_)))
                                });
                            }
                        }
                        let value_sentinel = make::new(
                            runner.arena_mut(),
                            ValueKind::Bool(false),
                            Rc::new(typ::make::bool().node),
                            Span::default(),
                        )
                        .unwrap();
                        records.push((pending_record(&result), value_sentinel));
                    }
                    assert_eq!(
                        records[0], records[1],
                        "{scenario:?} det={det} cache={cache} repeat={repeat}"
                    );
                }
            }
        }
    }
}

#[test]
fn pending_conditions_reach_otherwise_and_table_boundaries() {
    use p4spectec::interp::sl::eval::call::invoke_func;
    for table in [false, true] {
        for fallback in [None, Some(false), Some(true)] {
            let global =
                Global::load(pending_spec(PendingScenario::Deep, fallback, table)).unwrap();
            let ctx_root = Context::new(&global);
            let id_fixture = phrase!(node: Rc::from("fixture"), span: pending_span(50));
            let func = ctx_root.find_func(&id_fixture).unwrap();
            let id = phrase!(node: Rc::from("boundary"), span: pending_span(50));
            // Keep both aliases local and only the original syntax registered
            let mut ctx = ctx_root.clone();
            ctx.add_func(id.clone(), Rc::clone(func)).unwrap();
            let mut ctx_reference = ctx_root.clone();
            ctx_reference
                .add_func(id.clone(), Rc::new((**func).clone()))
                .unwrap();
            for det in [false, true] {
                let mut runner =
                    runner::build_sl(vec![], Config::new(false, det, false), NullExtern).unwrap();
                let mut runner_reference =
                    runner::build_sl(vec![], Config::new(false, det, false), NullExtern).unwrap();
                for repeat in 0..3 {
                    if repeat == 2 {
                        runner.reset();
                        runner_reference.reset();
                    }
                    let mut records = Vec::new();
                    for (runner, ctx) in
                        [(&mut runner, &ctx), (&mut runner_reference, &ctx_reference)]
                    {
                        let value =
                            make::nat(runner.arena_mut(), 7_u64.into(), Span::default()).unwrap();
                        let result = invoke_func(&mut runner.context(), ctx, &id, &[], &[value]);
                        if fallback == Some(true) {
                            assert!(matches!(&result, Ok(value_result) if *value_result == value));
                        } else {
                            let Err(runner::InterpreterError::Mismatch(reports)) = &result else {
                                panic!("expected a callable mismatch");
                            };
                            assert_eq!(reports.len(), 1);
                            let len = if !table && fallback == Some(false) {
                                0
                            } else if !table && det {
                                3
                            } else {
                                1
                            };
                            assert_eq!(
                                reports[0].children.len(),
                                len,
                                "table={table} fallback={fallback:?} det={det} top={:?} children={:?}",
                                reports[0].kind,
                                reports[0].children
                            );
                        }
                        let result =
                            result.map(|value| Flow::Return(phrase!(node: value, span: id.span)));
                        let value_sentinel = make::new(
                            runner.arena_mut(),
                            ValueKind::Bool(false),
                            Rc::new(typ::make::bool().node),
                            Span::default(),
                        )
                        .unwrap();
                        records.push((pending_record(&result), value_sentinel));
                    }
                    assert_eq!(
                        records[0], records[1],
                        "table={table} fallback={fallback:?} det={det} repeat={repeat}"
                    );
                }
            }
        }
    }
}
