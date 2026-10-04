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
