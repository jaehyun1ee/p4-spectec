use p4spec_rust::{
    lang::{
        common::source::Span,
        data::value::{ValueArena, make},
    },
    sim_plugin::{
        io::{Expectation, Transmission},
        runner::{Run, StfFailure},
        state::SimState,
    },
};

fn tx(port: i64, packet: &str) -> Transmission {
    Transmission {
        port,
        packet: packet.to_owned(),
    }
}

fn run() -> Run {
    let mut arena = ValueArena::new();
    let value = make::bool(&mut arena, false, Span::default()).unwrap();
    Run::new(SimState {
        value_ctx: value,
        value_arch: value,
        txs: vec![],
    })
}

#[test]
fn test_output_matches_only_first_transmission() {
    let mut run = run();
    run.on_tx_expect(Expectation {
        tx: tx(1, "AA"),
        exact: false,
    })
    .unwrap();
    run.on_tx_expect(Expectation {
        tx: tx(2, "BB"),
        exact: true,
    })
    .unwrap();
    run.state.txs = vec![tx(1, "AACC"), tx(2, "BB")];
    assert_eq!(run.on_tx_output().unwrap(), Some(tx(1, "AA")));
    assert_eq!(run.tx_output_queue, vec![tx(2, "BB")]);
    assert_eq!(run.expect_queue.len(), 1);
    assert!(matches!(run.finish(), Err(StfFailure::Remaining { .. })));
}

#[test]
fn test_first_same_port_mismatch_preserves_queues() {
    let mut run = run();
    run.on_tx_expect(Expectation {
        tx: tx(1, "BB"),
        exact: true,
    })
    .unwrap();
    run.on_tx_expect(Expectation {
        tx: tx(1, "AA"),
        exact: true,
    })
    .unwrap();
    run.state.txs = vec![tx(1, "AA")];
    assert!(matches!(
        run.on_tx_output(),
        Err(StfFailure::Mismatch { .. })
    ));
    assert_eq!(run.expect_queue.len(), 2);
    let mut run = self::run();
    run.state.txs = vec![tx(1, "BB"), tx(1, "AA")];
    run.on_tx_output().unwrap();
    assert!(matches!(
        run.on_tx_expect(Expectation {
            tx: tx(1, "AA"),
            exact: true
        }),
        Err(StfFailure::Mismatch { .. })
    ));
    assert_eq!(run.tx_output_queue.len(), 2);
}

#[test]
fn test_output_before_expect_logs_actual_and_preserves_other_ports() {
    let mut run = run();
    run.state.txs = vec![tx(2, "BB"), tx(1, "AACC")];
    assert_eq!(run.on_tx_output().unwrap(), None);
    assert_eq!(
        run.on_tx_expect(Expectation {
            tx: tx(1, "A*"),
            exact: false
        })
        .unwrap(),
        Some(tx(1, "AACC"))
    );
    assert_eq!(run.tx_output_queue, vec![tx(2, "BB")]);
    assert_eq!(
        run.on_tx_expect(Expectation {
            tx: tx(2, ""),
            exact: false
        })
        .unwrap(),
        Some(tx(2, "BB"))
    );
    run.finish().unwrap();
}

#[test]
fn test_dropped_packet_retains_expectation() {
    let mut run = run();
    run.on_tx_expect(Expectation {
        tx: tx(1, ""),
        exact: false,
    })
    .unwrap();
    assert_eq!(run.on_tx_output().unwrap(), None);
    assert!(matches!(run.finish(), Err(StfFailure::Remaining { .. })));
}

use p4spec_rust::{
    interp::al::error::Error as InterpError,
    lang::{
        data::{
            typ,
            value::{Value, get},
        },
        il::ast::Typ,
        xl::num,
    },
    runner::{Extern, Interface, Interpreter, NullInterface, Runner, RunnerContext},
    sim_plugin::{
        ebpf::Ebpf,
        runner::{self, Error},
    },
    stf::{
        self,
        ast::{Action, Argument, MatchKind, Statement, TableMatch},
    },
};

#[derive(Default)]
struct StfInterp {
    calls: Vec<(String, Vec<Value>)>,
    initialized: bool,
}

impl<Iface: Interface, Exn: Extern> Interpreter<Iface, Exn> for StfInterp {
    type Spec = ();
    type Error = InterpError;
    fn clear(&mut self) {
        self.calls.clear();
    }
    fn reset(&mut self) {
        self.calls.clear();
        self.initialized = false;
    }
    fn eval_program(
        ctx: &mut RunnerContext<'_, Self, Iface, Exn>,
        name: &str,
        _program: Value,
    ) -> Result<Vec<Value>, InterpError> {
        assert_eq!(name, "EBPF_init");
        assert!(
            !ctx.interp().initialized,
            "previous run's interpreter state was not reset"
        );
        ctx.interp_mut().initialized = true;
        let value = make::bool(ctx.arena_mut(), false, Span::default())?;
        Ok(vec![value, value])
    }
    fn eval_rel(
        _ctx: &mut RunnerContext<'_, Self, Iface, Exn>,
        _name: &str,
        _values: &[Value],
    ) -> Result<Vec<Value>, InterpError> {
        unreachable!()
    }
    fn eval_func(
        ctx: &mut RunnerContext<'_, Self, Iface, Exn>,
        name: &str,
        _targs: &[Typ],
        values: &[Value],
    ) -> Result<Value, InterpError> {
        ctx.interp_mut()
            .calls
            .push((name.to_owned(), values.to_vec()));
        match name {
            "find_object_unqualified_e" => Ok(make::opt(
                ctx.arena_mut(),
                typ::make::opt(typ::make::bool()).node.into(),
                Some(values[0]),
                Span::default(),
            )?),
            "tableObject_add_entry" => {
                let value = make::tuple(
                    ctx.arena_mut(),
                    typ::make::bool().node.into(),
                    values[1..].to_vec(),
                    Span::default(),
                )?;
                Ok(make::opt(
                    ctx.arena_mut(),
                    typ::make::opt(typ::make::bool()).node.into(),
                    Some(value),
                    Span::default(),
                )?)
            }
            "tableObject_add_default_action" => Ok(make::tuple(
                ctx.arena_mut(),
                typ::make::bool().node.into(),
                values[1..].to_vec(),
                Span::default(),
            )?),
            "update_object_unqualified_e" => Ok(values[2]),
            _ => panic!("unexpected specification call {name}"),
        }
    }
}

fn stf_runner() -> (Runner<StfInterp, NullInterface, Ebpf>, Run) {
    let mut runner = Runner::new((), StfInterp::default(), NullInterface, Ebpf);
    let value = make::bool(runner.arena_mut(), false, Span::default()).unwrap();
    let run = Run::new(SimState {
        value_ctx: value,
        value_arch: value,
        txs: vec![],
    });
    (runner, run)
}

fn statement(stmt: Statement) -> p4spec_rust::lang::common::source::Phrase<Statement> {
    p4spec_rust::phrase!(node: stmt, span: Span::default())
}

#[test]
fn test_ordered_table_encoding_and_register_failure() {
    let (mut runner, mut run) = stf_runner();
    let action = Action {
        name: "pipe_act".into(),
        args: vec![
            Argument {
                id: "second".into(),
                num: "0x7fffffffffffffff".into(),
            },
            Argument {
                id: "first".into(),
                num: "0b10".into(),
            },
        ],
    };
    let stmt = statement(Statement::Add {
        table: "tab\"".into(),
        priority: Some(7),
        matches: vec![
            TableMatch {
                name: "hdr$12.field$0".into(),
                kind: MatchKind::Number("0xF*".into()),
            },
            TableMatch {
                name: "bin".into(),
                kind: MatchKind::Number("0b10".into()),
            },
            TableMatch {
                name: "upper".into(),
                kind: MatchKind::Number("0XFF".into()),
            },
            TableMatch {
                name: "prefix".into(),
                kind: MatchKind::Slash("0xAB".into(), "0b1000".into()),
            },
        ],
        action: action.clone(),
        id: Some("ignored".into()),
    });
    runner::step(&mut runner, &mut run, &stmt).unwrap();
    let value_added = run.state.value_arch;
    let values = get::tuple(runner.arena(), &value_added).unwrap();
    let value_priority = get::opt(runner.arena(), &values[1]).unwrap().unwrap();
    assert_eq!(
        num::to_int(get::num(runner.arena(), &value_priority).unwrap()),
        &7.into()
    );
    let values_key = get::list(runner.arena(), &values[2]).unwrap();
    let values_key = values_key
        .iter()
        .map(|value| get::tuple(runner.arena(), value).unwrap())
        .collect::<Vec<_>>();
    assert_eq!(
        get::text(runner.arena(), &values_key[0][0]).unwrap(),
        "hdr[12].field[0]"
    );
    for (values, shape, text) in values_key[..3]
        .iter()
        .zip(["_HEX text", "_BIN text", "_DEC text"])
        .zip(["F*", "10", "0XFF"])
        .map(|((values, shape), text)| (values, shape, text))
    {
        let case = get::case(runner.arena(), &values[1]).unwrap();
        let shape = p4spec_rust::frontend::parse::parse_mixop(shape).unwrap();
        assert!(case.eq_shape(&shape));
        assert_eq!(get::text(runner.arena(), case.args()[0]).unwrap(), text);
    }
    let case = get::case(runner.arena(), &values_key[3][1]).unwrap();
    let shape = p4spec_rust::frontend::parse::parse_mixop("text _SLASH nat").unwrap();
    assert!(case.eq_shape(&shape));
    let values = case.args();
    assert_eq!(get::text(runner.arena(), values[0]).unwrap(), "0xAB");
    assert_eq!(
        num::to_int(get::num(runner.arena(), values[1]).unwrap()),
        &8.into()
    );
    let values = get::tuple(runner.arena(), &value_added).unwrap();
    let values_action = get::tuple(runner.arena(), &values[3]).unwrap();
    assert_eq!(get::text(runner.arena(), &values_action[0]).unwrap(), "act");
    let values_arg = get::list(runner.arena(), &values_action[1]).unwrap();
    let values_arg = values_arg
        .iter()
        .map(|value| get::tuple(runner.arena(), value).unwrap())
        .collect::<Vec<_>>();
    assert_eq!(
        get::text(runner.arena(), &values_arg[0][0]).unwrap(),
        "second"
    );
    assert_eq!(
        num::to_int(get::num(runner.arena(), &values_arg[0][1]).unwrap()),
        &(-1).into()
    );
    assert_eq!(
        get::text(runner.arena(), &values_arg[1][0]).unwrap(),
        "first"
    );
    let calls = runner.context().interp().calls.clone();
    assert_eq!(
        get::text(runner.arena(), &calls[0].1[1]).unwrap(),
        "tab\\\""
    );
    runner::step(
        &mut runner,
        &mut run,
        &statement(Statement::SetDefault {
            table: "tab\"".into(),
            action,
        }),
    )
    .unwrap();
    let values = get::tuple(runner.arena(), &run.state.value_arch).unwrap();
    assert_eq!(values[0], value_added);
    let calls = runner.context().interp().calls.clone();
    assert_eq!(get::text(runner.arena(), &calls[3].1[1]).unwrap(), "tab\"");
    let value_default = run.state.value_arch;
    let error = runner::step(
        &mut runner,
        &mut run,
        &statement(Statement::RegisterWrite {
            name: "r".into(),
            index: "0".into(),
            value: "1".into(),
        }),
    )
    .unwrap_err();
    assert!(
        error
            .to_string()
            .contains("register_write is not implemented for the ebpf simulator")
    );
    assert_eq!(run.state.value_arch, value_default);
}

#[test]
fn test_native_steps_clear_raw_outputs_without_flushing_pending_queues() {
    let (mut runner, mut run) = stf_runner();
    run.state.txs = vec![tx(1, "AAFF")];
    run.on_tx_output().unwrap();
    let stmts = stf::parse::parse_str(
        "commands.stf",
        "wait\nmirroring_get 4611686018427387904\nexpect 1 aa*\nno_packet\n",
    )
    .unwrap();
    runner::step(&mut runner, &mut run, &stmts[0]).unwrap();
    assert!(run.state.txs.is_empty());
    assert_eq!(run.tx_output_queue, vec![tx(1, "AAFF")]);
    runner::step(&mut runner, &mut run, &stmts[1]).unwrap();
    assert_eq!(
        runner::step(&mut runner, &mut run, &stmts[2]).unwrap(),
        Some(tx(1, "AAFF"))
    );
    assert_eq!(run.matches, vec![tx(1, "AAFF")]);
    assert!(
        matches!(runner::step(&mut runner, &mut run, &stmts[3]), Err(Error::Stf { failure, span }) if matches!(*failure, StfFailure::Unsupported(_)) && span == stmts[3].span)
    );
    run.finish().unwrap();
}

#[test]
fn test_integer_failure_is_located_and_precedes_pipeline_dispatch() {
    let (mut runner, mut run) = stf_runner();
    for source in [
        "packet 4611686018427387904 AA",
        "expect 4611686018427387904 AA",
        "register_write r 0 0x****************",
    ] {
        let stmts = stf::parse::parse_str("overflow.stf", source).unwrap();
        assert!(
            matches!(runner::step(&mut runner, &mut run, &stmts[0]), Err(Error::Runtime(error)) if error.span == stmts[0].span)
        );
    }
    assert!(runner.context().interp().calls.is_empty());
}

#[test]
fn test_fresh_run_resets_interpreter_state_and_queues() {
    let (mut runner, _) = stf_runner();
    let path = std::env::temp_dir().join(format!("p4spec-stf-reset-{}.p4", std::process::id()));
    std::fs::write(&path, "").unwrap();
    let mut run = runner::init_pipe(&mut runner, &[], &path).unwrap();
    run.on_tx_expect(Expectation {
        tx: tx(1, "AA"),
        exact: true,
    })
    .unwrap();
    let run = runner::init_pipe(&mut runner, &[], &path).unwrap();
    std::fs::remove_file(path).unwrap();
    assert!(run.state.txs.is_empty());
    assert!(run.matches.is_empty());
    run.finish().unwrap();
}
