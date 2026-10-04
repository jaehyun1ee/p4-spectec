use std::{borrow::Cow, rc::Rc};

use p4spectec::{
    diagnostic::{Report, ReportKind},
    interp::{
        shared::context::{ReadContext, WriteContext},
        sl::{
            context::{Context, Global},
            eval::instr::eval_instr,
            flow::Flow,
        },
    },
    lang::{
        common::source::{FileId, Position, Span},
        data::{
            typ,
            value::{Arena, Value, ValueKind, get, make},
        },
        il::ast as source,
        sl::ast as sl_source,
        traits::print::Print,
    },
    note_phrase, phrase,
    runner::{self, Config, InterpreterError, NullExtern},
    runtime::envs::interp::sl::ast_prepared as ast,
};

#[derive(Clone, Copy, Debug)]
enum Scenario {
    Return,
    Tail,
    Condition,
    Mismatch,
    MultiMismatch,
    BindingMismatch,
    Fatal,
    Empty,
}

fn span(line: usize) -> Span {
    let file = FileId::intern("instruction-chain.watsup");
    Span::new(Position::new(file, line, 1), Position::new(file, line, 8))
}

fn id_exp(line: usize) -> source::Exp {
    note_phrase!(
        node: source::ExpKind::Id(phrase!(node: Rc::from("n"), span: span(line))),
        note: typ::make::nat().node,
        span: span(line)
    )
}

fn let_instr(exp_r: source::Exp, block: sl_source::Block, line: usize) -> sl_source::Instr {
    phrase!(node: sl_source::InstrKind::Let(sl_source::LetInstr {
        exp_l: id_exp(line), exp_r, iter_instrs: vec![], block,
    }), span: span(line))
}

/// Pads each continuation with an empty identity binding to keep recursive choice.
fn wrap(block: sl_source::Block, padded: bool, line: usize) -> sl_source::Instr {
    let mut block = block;
    if padded {
        block.insert(0, let_instr(id_exp(line), vec![], line));
    }
    let_instr(id_exp(line), block, line)
}

/// Builds chains with independent expected terminal and failure behavior.
fn fixture(scenario: Scenario, padded: bool, depth: usize) -> Global {
    let mut spec = super::support::sl_spec(
        "var n : nat\n\
         dec $identity(nat) : nat\n\
         def $identity(n) = n\n\
         dec $failure_nat(nat) : nat\n\
         def $failure_nat(n) = n\n\
         dec $failure_bool() : bool\n\
         def $failure_bool() = false\n\
         dec $fixture(nat) : nat\n\
         def $fixture(n) = $identity(n)\n",
    );
    for def in &mut spec {
        let sl_source::DefKind::MetaFunc(sl_source::MetaFuncDef::Defined(func)) = &mut def.node
        else {
            continue;
        };
        if matches!(func.id.node.as_ref(), "failure_nat" | "failure_bool") {
            func.block.clear();
            func.block_else = None;
        }
        if func.id.node.as_ref() != "fixture" {
            continue;
        }
        let sl_source::InstrKind::Return(instr_call) = &func.block[0].node else { unreachable!() };
        let mut exp_call = instr_call.exp.clone();
        exp_call.span = span(40);
        let instr_return = phrase!(
            node: sl_source::InstrKind::Return(sl_source::ReturnInstr { exp: id_exp(40) }),
            span: span(40)
        );
        let exp_failure = note_phrase!(
            node: source::ExpKind::Call(
                phrase!(node: Rc::from("failure_bool"), span: span(50)), vec![], vec![]
            ),
            note: typ::make::bool().node,
            span: span(50)
        );
        let instr_failure = phrase!(
            node: sl_source::InstrKind::If(sl_source::IfInstr {
                exp: exp_failure, iter_exps: vec![], block: vec![], dangle: false,
            }),
            span: span(50)
        );
        let mut block = match scenario {
            Scenario::Return => vec![instr_return],
            Scenario::Tail => vec![phrase!(
                node: sl_source::InstrKind::Return(sl_source::ReturnInstr { exp: exp_call }),
                span: span(40)
            )],
            Scenario::Condition => {
                let exp = note_phrase!(
                    node: source::ExpKind::Bool(false),
                    note: typ::make::bool().node,
                    span: span(50)
                );
                vec![phrase!(
                    node: sl_source::InstrKind::If(sl_source::IfInstr {
                        exp, iter_exps: vec![], block: vec![], dangle: false,
                    }),
                    span: span(50)
                )]
            }
            Scenario::Mismatch => vec![instr_failure],
            Scenario::MultiMismatch => {
                let exp = note_phrase!(
                    node: source::ExpKind::Bool(true),
                    note: typ::make::bool().node,
                    span: span(45)
                );
                vec![phrase!(
                    node: sl_source::InstrKind::If(sl_source::IfInstr {
                        exp, iter_exps: vec![], block: vec![instr_failure, instr_return], dangle: false,
                    }),
                    span: span(45)
                )]
            }
            Scenario::BindingMismatch => {
                let source::ExpKind::Call(id, _, _) = &mut exp_call.node else { unreachable!() };
                id.node = Rc::from("failure_nat");
                id.span = span(50);
                vec![let_instr(exp_call, vec![instr_return], 50)]
            }
            Scenario::Fatal => {
                let source::ExpKind::Call(id, _, _) = &mut exp_call.node else { unreachable!() };
                id.node = Rc::from("undefined");
                id.span = span(50);
                // Keep the failing lookup below a binding, outside tail-call handling
                vec![let_instr(exp_call, vec![instr_return], 50)]
            }
            Scenario::Empty => vec![],
        };
        for line in (1..=depth).rev() {
            block = vec![wrap(block, padded, line)];
        }
        // An outer binding changes only this chain's owned branch context
        let exp = note_phrase!(
            node: source::ExpKind::Num(source::Num::Nat(9_u64.into())),
            note: typ::make::nat().node,
            span: span(0)
        );
        func.block = vec![let_instr(exp, block, 0)];
    }
    Global::load(spec).unwrap()
}

fn report_record(report: &Report) -> String {
    format!(
        "{:?} {:?}",
        report.kind,
        report
            .children
            .iter()
            .map(report_record)
            .collect::<Vec<_>>()
    )
}

fn record(result: &Result<Flow, InterpreterError>) -> String {
    match result {
        Ok(Flow::Cont(reports)) => {
            format!("Cont {:?}", reports.iter().map(report_record).collect::<Vec<_>>())
        }
        Err(InterpreterError::Mismatch(reports)) => {
            format!("Mismatch {:?}", reports.iter().map(report_record).collect::<Vec<_>>())
        }
        Err(InterpreterError::Fatal(report)) => format!("Fatal {}", report_record(report)),
        result => format!("{result:?}"),
    }
}

fn sentinel(arena: &mut Arena) -> Value {
    make::new(
        arena,
        ValueKind::Text("after chain".into()),
        Rc::new(typ::make::text().node),
        span(90),
    )
    .unwrap()
}

/// Checks the intended result independently of the padded reference.
fn check_result(
    arena: &Arena,
    result: &Result<Flow, InterpreterError>,
    scenario: Scenario,
    det: bool,
    tail: bool,
) {
    match scenario {
        Scenario::Return | Scenario::MultiMismatch
            if matches!(scenario, Scenario::Return) || det =>
        {
            let Ok(Flow::Return(value)) = result else { panic!("{}", record(result)) };
            assert_eq!(get::num(arena, &value.node).unwrap().to_string(), "9");
            assert_eq!(value.span, span(40));
        }
        Scenario::Tail if tail => {
            let Ok(Flow::TailFunc(call)) = result else { panic!("{}", record(result)) };
            assert_eq!(call.node.0.node.as_ref(), "identity");
            assert_eq!(call.span, span(40));
            assert_eq!(get::num(arena, &call.node.2[0]).unwrap().to_string(), "9");
        }
        Scenario::Tail => {
            let Ok(Flow::Return(value)) = result else { panic!("{}", record(result)) };
            assert_eq!(get::num(arena, &value.node).unwrap().to_string(), "9");
            assert_eq!(value.span, span(40));
        }
        Scenario::Fatal => assert!(matches!(result, Err(InterpreterError::Fatal(_)))),
        Scenario::Condition => {
            let Ok(Flow::Cont(reports)) = result else { panic!("{}", record(result)) };
            assert_eq!(reports.len(), 1);
            let ReportKind::Cause(diagnostic) = &reports[0].kind else { unreachable!() };
            assert_eq!(diagnostic.labels[0].span, span(50));
        }
        Scenario::Empty | Scenario::Mismatch if matches!(scenario, Scenario::Empty) || det => {
            let Ok(Flow::Cont(reports)) = result else { panic!("{}", record(result)) };
            assert!(reports.is_empty());
        }
        Scenario::Mismatch | Scenario::MultiMismatch | Scenario::BindingMismatch => {
            let Ok(Flow::Cont(reports)) = result else { panic!("{}", record(result)) };
            assert_eq!(reports.len(), 1);
            assert!(record(result).contains("failure_"));
        }
        Scenario::Return | Scenario::Empty => unreachable!(),
    }
}

#[test]
fn chained_bindings_keep_mismatch_boundaries_tail_spans_and_owned_scopes() {
    for scenario in [
        Scenario::Return,
        Scenario::Tail,
        Scenario::Condition,
        Scenario::Mismatch,
        Scenario::MultiMismatch,
        Scenario::BindingMismatch,
        Scenario::Fatal,
        Scenario::Empty,
    ] {
        let globals = [fixture(scenario, false, 3), fixture(scenario, true, 3)];
        for det in [false, true] {
            for tail in [false, true] {
                for borrowed in [false, true] {
                    let mut records = Vec::new();
                    for global in &globals {
                        let ctx_root = Context::new(global);
                        let id = phrase!(node: Rc::from("fixture"), span: Span::default());
                        let func = ctx_root.find_func(&id).unwrap();
                        let ast::MetaFuncDef::Defined(func_def) = &func.def else { unreachable!() };
                        let mut layout = (*func.layout).clone();
                        let id_param = layout.resolve_id(phrase!(
                            node: Rc::from("n"), span: Span::default()
                        ));
                        assert_eq!(layout.len(), func.layout.len());
                        let mut runner =
                            runner::build_sl(vec![], Config::new(true, det, false), NullExtern)
                                .unwrap();
                        for repeat in 0..3 {
                            if repeat == 2 {
                                runner.reset();
                            }
                            let value =
                                make::nat(runner.arena_mut(), 7_u64.into(), Span::default())
                                    .unwrap();
                            let mut ctx = ctx_root.localize_with_layout(&func.layout);
                            ctx.add_value_at_slot(id_param.slot, value);
                            let ctx_saved = ctx.clone();
                            let ctx =
                                if borrowed { Cow::Borrowed(&ctx_saved) } else { Cow::Owned(ctx) };
                            let result =
                                eval_instr(&mut runner.context(), ctx, &func_def.block[0], tail);
                            check_result(runner.arena(), &result, scenario, det, tail);
                            assert_eq!(ctx_saved.find_value_at_slot(id_param.slot), Some(&value));
                            records.push((record(&result), sentinel(runner.arena_mut())));
                        }
                    }
                    assert_eq!(
                        records[..3],
                        records[3..],
                        "{scenario:?}, det {det}, tail {tail}, borrowed {borrowed}"
                    );
                }
            }
        }
    }
}

/// Catches a failing root binder before any successful chain sets the boundary.
#[test]
fn a_failing_root_binding_converts_its_own_mismatch() {
    let global = fixture(Scenario::BindingMismatch, false, 0);
    let ctx_root = Context::new(&global);
    let id = phrase!(node: Rc::from("fixture"), span: Span::default());
    let func = ctx_root.find_func(&id).unwrap();
    let ast::MetaFuncDef::Defined(func_def) = &func.def else { unreachable!() };
    let ast::InstrKind::Let(instr_outer) = &func_def.block[0].node else { unreachable!() };
    // Enter the failing binder directly, before the outer assignment of nine
    let instr = &instr_outer.block[0];
    let ast::InstrKind::Let(instr_let) = &instr.node else { unreachable!() };
    assert_eq!(instr_let.block.len(), 1);
    let mut layout = (*func.layout).clone();
    let id_param = layout.resolve_id(phrase!(node: Rc::from("n"), span: Span::default()));
    assert_eq!(layout.len(), func.layout.len());
    for det in [false, true] {
        let mut runner =
            runner::build_sl(vec![], Config::new(false, det, false), NullExtern).unwrap();
        let value = make::nat(runner.arena_mut(), 7_u64.into(), Span::default()).unwrap();
        let mut ctx = ctx_root.localize_with_layout(&func.layout);
        ctx.add_value_at_slot(id_param.slot, value);
        let result = eval_instr(&mut runner.context(), Cow::Owned(ctx), instr, false);
        let Ok(Flow::Cont(reports)) = &result else { panic!("{}", record(&result)) };
        assert_eq!(reports.len(), 1);
        assert!(record(&result).contains("failure_nat"));
    }
}
