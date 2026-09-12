use p4spec_rust::{
    sim_plugin::{
        io::Transmission,
        psa::{self, Psa},
    },
    stf::ast::Statement,
};

#[path = "pipe/serde.rs"]
mod serde;

#[test]
fn test_native_psa_micro_fixture_packets() {
    let mut runner = super::super::runner(Psa);
    let path = super::super::repo().join("p4spec/test/micro/sim-psa/psa.p4");
    let program = super::super::parse_program(runner.arena_mut(), &path);
    let mut state = psa::init_pipe(&mut runner.context(), program).unwrap();
    let program_stf = p4spec_rust::stf::parse::parse_file(path.with_extension("stf")).unwrap();
    let mut txs = Vec::new();
    let mut txs_expect = Vec::new();
    for stmt in program_stf {
        match stmt.node {
            Statement::Packet { port, packet } => {
                psa::drive_pipe(
                    &mut runner.context(),
                    &mut state,
                    &Transmission {
                        port: port.parse().unwrap(),
                        packet,
                    },
                )
                .unwrap();
                txs.extend(state.txs.iter().map(|tx| (tx.port, tx.packet.clone())));
            }
            Statement::Expect {
                port,
                packet_expected: Some(packet),
                ..
            } => txs_expect.push((port.parse::<i64>().unwrap(), packet.to_ascii_uppercase())),
            _ => panic!("micro fixture contains packet and expectation statements"),
        }
    }
    assert_eq!(txs.len(), 4);
    assert_eq!(txs, txs_expect);
}

type Runner = p4spec_rust::runner::Runner<
    p4spec_rust::interp::al::AlInterp,
    p4spec_rust::runner::BuiltinInterface,
    Psa,
>;
use p4spec_rust::{
    lang::{
        common::source::Span,
        data::{
            typ,
            value::{Value, make},
        },
    },
    sim_plugin::{
        psa::{arch::Arch, packet::Entrypoint, pipe, scheduler},
        spec_impl::{pack, rel, unpack},
        state::SimState,
    },
};

fn pipeline() -> (Runner, SimState) {
    let mut runner = super::super::runner(Psa);
    let path = super::super::repo().join("p4spec/test/micro/sim-psa/psa.p4");
    let program = super::super::parse_program(runner.arena_mut(), &path);
    let mut state = psa::init_pipe(&mut runner.context(), program).unwrap();
    psa::drive_pipe(
        &mut runner.context(),
        &mut state,
        &Transmission {
            port: 4,
            packet: "000000000001000000000000FFFF".to_owned(),
        },
    )
    .unwrap();
    (runner, state)
}

fn write(runner: &mut Runner, state: &mut SimState, metadata: &str, field: &str, value: Value) {
    state.value_ctx = rel::lvalue_write_dot_global(
        &mut runner.context(),
        state.value_ctx,
        state.value_arch,
        metadata,
        field,
        value,
    )
    .unwrap();
}

fn write_bool(
    runner: &mut Runner,
    state: &mut SimState,
    metadata: &str,
    field: &str,
    boolean: bool,
) {
    let value = make::bool(runner.arena_mut(), boolean, Span::default()).unwrap();
    let mixop = p4spec_rust::frontend::parse::parse_mixop("_B bool").unwrap();
    let mixfix =
        p4spec_rust::lang::common::notation::mixop::Mixop::fill(&mixop, vec![value]).unwrap();
    let typ = typ::make::var(
        p4spec_rust::phrase!(node: "value".to_owned(), span: Span::default()),
        vec![],
    );
    let value = make::case(runner.arena_mut(), typ.node.into(), mixfix, Span::default()).unwrap();
    write(runner, state, metadata, field, value);
}

fn write_int(
    runner: &mut Runner,
    state: &mut SimState,
    metadata: &str,
    field: &str,
    width: i64,
    int: i64,
) {
    let value = pack::p4_fixed_bit(runner.arena_mut(), width.into(), int.into()).unwrap();
    write(runner, state, metadata, field, value);
}

fn read_int(
    runner: &mut Runner,
    value_ctx: Value,
    value_arch: Value,
    metadata: &str,
    field: &str,
) -> i64 {
    let value = rel::lvalue_read_dot_global(
        &mut runner.context(),
        value_ctx,
        value_arch,
        metadata,
        field,
    )
    .unwrap();
    unpack::signed_int(&unpack::p4_fixed_bit(runner.arena(), &value).unwrap().int).unwrap()
}

fn read_path(runner: &mut Runner, value_ctx: Value, value_arch: Value, metadata: &str) -> String {
    let value = rel::lvalue_read_dot_global(
        &mut runner.context(),
        value_ctx,
        value_arch,
        metadata,
        "packet_path",
    )
    .unwrap();
    unpack::p4_enum(runner.arena(), &value).unwrap().1
}

fn configure_mirror(runner: &mut Runner, state: &mut SimState) {
    state.value_arch = pipe::mc_mgrp_create(&mut runner.context(), state.value_arch, 7).unwrap();
    state.value_arch =
        pipe::mc_node_create(&mut runner.context(), state.value_arch, 42, &[12, 3]).unwrap();
    state.value_arch =
        pipe::mc_node_associate(&mut runner.context(), state.value_arch, 7, 0).unwrap();
    state.value_arch =
        pipe::add_mirror_session_mc(&mut runner.context(), state.value_arch, 5, 7).unwrap();
}

#[test]
fn test_clone_survives_ingress_drop() {
    let (mut runner, mut state) = pipeline();
    configure_mirror(&mut runner, &mut state);
    write_bool(
        &mut runner,
        &mut state,
        "ingress_output_metadata",
        "clone",
        true,
    );
    write_int(
        &mut runner,
        &mut state,
        "ingress_output_metadata",
        "clone_session_id",
        16,
        5,
    );
    write_bool(
        &mut runner,
        &mut state,
        "ingress_output_metadata",
        "drop",
        true,
    );
    let value_ctx_original = state.value_ctx;
    let value_arch_original = state.value_arch;
    pipe::run_pre(&mut runner.context(), &mut state).unwrap();
    assert_eq!(state.value_ctx, value_ctx_original);
    let arch = pipe::get_arch_state(&mut runner.context(), state.value_arch).unwrap();
    assert_eq!(arch.queue.len(), 2);
    for (packet, port) in arch.queue.iter().zip([12, 3]) {
        assert_eq!(packet.entrypoint, Entrypoint::Egress);
        assert_eq!(packet.packet_in.idx, 0);
        assert_eq!(
            read_int(
                &mut runner,
                packet.value_ctx,
                state.value_arch,
                "egress_input_metadata",
                "egress_port"
            ),
            port
        );
        assert_eq!(
            read_path(
                &mut runner,
                packet.value_ctx,
                state.value_arch,
                "egress_input_metadata"
            ),
            "CLONE_I2E"
        );
    }
    let arch_original = pipe::get_arch_state(&mut runner.context(), value_arch_original).unwrap();
    let value_arch_restored =
        pipe::set_arch_state(&mut runner.context(), state.value_arch, &arch_original).unwrap();
    assert_eq!(
        p4spec_rust::lang::data::value::serde::encode(runner.arena(), &value_arch_restored)
            .unwrap(),
        p4spec_rust::lang::data::value::serde::encode(runner.arena(), &value_arch_original)
            .unwrap()
    );
    state.txs.clear();
    scheduler::run_scheduler(&mut runner.context(), &mut state).unwrap();
    assert_eq!(
        state.txs.iter().map(|tx| tx.port).collect::<Vec<_>>(),
        [12, 3]
    );
    let txs: Vec<_> = state
        .txs
        .iter()
        .map(|tx| (tx.port, tx.packet.clone()))
        .collect();
    let value_ctx = state.value_ctx;
    scheduler::run_scheduler(&mut runner.context(), &mut state).unwrap();
    assert_eq!(
        state
            .txs
            .iter()
            .map(|tx| (tx.port, tx.packet.clone()))
            .collect::<Vec<_>>(),
        txs
    );
    assert_eq!(state.value_ctx, value_ctx);
}

#[test]
fn test_resubmit_and_recirculate_preserve_queue_order() {
    let (mut runner, mut state) = pipeline();
    configure_mirror(&mut runner, &mut state);
    write_bool(
        &mut runner,
        &mut state,
        "ingress_output_metadata",
        "clone",
        true,
    );
    write_int(
        &mut runner,
        &mut state,
        "ingress_output_metadata",
        "clone_session_id",
        16,
        5,
    );
    write_bool(
        &mut runner,
        &mut state,
        "ingress_output_metadata",
        "drop",
        false,
    );
    write_bool(
        &mut runner,
        &mut state,
        "ingress_output_metadata",
        "resubmit",
        true,
    );
    write_int(
        &mut runner,
        &mut state,
        "ingress_output_metadata",
        "multicast_group",
        16,
        7,
    );
    pipe::run_pre(&mut runner.context(), &mut state).unwrap();
    let arch = pipe::get_arch_state(&mut runner.context(), state.value_arch).unwrap();
    assert_eq!(
        arch.queue
            .iter()
            .map(|packet| packet.entrypoint)
            .collect::<Vec<_>>(),
        [Entrypoint::Egress, Entrypoint::Egress, Entrypoint::Ingress]
    );
    let packet = &arch.queue[2];
    assert_eq!(
        read_path(
            &mut runner,
            packet.value_ctx,
            state.value_arch,
            "ingress_input_metadata"
        ),
        "RESUBMIT"
    );
    assert_eq!(
        read_int(
            &mut runner,
            packet.value_ctx,
            state.value_arch,
            "ingress_input_metadata",
            "ingress_port"
        ),
        4
    );
    assert_eq!(packet.packet_in.idx, 0);

    write_bool(
        &mut runner,
        &mut state,
        "egress_output_metadata",
        "clone",
        true,
    );
    write_int(
        &mut runner,
        &mut state,
        "egress_output_metadata",
        "clone_session_id",
        16,
        5,
    );
    write_bool(
        &mut runner,
        &mut state,
        "egress_output_metadata",
        "drop",
        false,
    );
    write_int(
        &mut runner,
        &mut state,
        "egress_input_metadata",
        "egress_port",
        32,
        0xfffffffa,
    );
    pipe::run_bqe(&mut runner.context(), &mut state).unwrap();
    let arch = pipe::get_arch_state(&mut runner.context(), state.value_arch).unwrap();
    assert_eq!(arch.queue.len(), 6);
    assert_eq!(
        read_path(
            &mut runner,
            arch.queue[3].value_ctx,
            state.value_arch,
            "egress_input_metadata"
        ),
        "CLONE_E2E"
    );
    assert_eq!(
        read_path(
            &mut runner,
            arch.queue[5].value_ctx,
            state.value_arch,
            "ingress_input_metadata"
        ),
        "RECIRCULATE"
    );
    assert_eq!(
        read_int(
            &mut runner,
            arch.queue[5].value_ctx,
            state.value_arch,
            "ingress_input_metadata",
            "ingress_port"
        ),
        0xfffffffa
    );

    state.value_arch =
        pipe::set_arch_state(&mut runner.context(), state.value_arch, &Arch::default()).unwrap();
    write_bool(
        &mut runner,
        &mut state,
        "egress_output_metadata",
        "clone",
        false,
    );
    write_int(
        &mut runner,
        &mut state,
        "egress_input_metadata",
        "egress_port",
        33,
        0xfffffffa,
    );
    let num_txs = state.txs.len();
    pipe::run_bqe(&mut runner.context(), &mut state).unwrap();
    assert_eq!(state.txs.len(), num_txs + 1);
    assert!(
        pipe::get_arch_state(&mut runner.context(), state.value_arch)
            .unwrap()
            .queue
            .is_empty()
    );
}

#[test]
fn test_multicast_restores_context_without_losing_effects() {
    let (mut runner, mut state) = pipeline();
    configure_mirror(&mut runner, &mut state);
    let value_ctx = state.value_ctx;
    pipe::schedule_multicast(&mut runner.context(), &mut state, 7).unwrap();
    assert_eq!(state.value_ctx, value_ctx);
    let arch = pipe::get_arch_state(&mut runner.context(), state.value_arch).unwrap();
    assert_eq!(arch.queue.len(), 2);
    for (packet, port) in arch.queue.iter().zip([12, 3]) {
        assert_eq!(
            read_path(
                &mut runner,
                packet.value_ctx,
                state.value_arch,
                "egress_input_metadata"
            ),
            "NORMAL_MULTICAST"
        );
        assert_eq!(
            read_int(
                &mut runner,
                packet.value_ctx,
                state.value_arch,
                "egress_input_metadata",
                "egress_port"
            ),
            port
        );
        assert_eq!(
            read_int(
                &mut runner,
                packet.value_ctx,
                state.value_arch,
                "egress_input_metadata",
                "instance"
            ),
            42
        );
    }
    assert_eq!(arch.multicast.groups[&7], [0]);
    assert_eq!(arch.mirrortable[&5], 7);
    let value_arch = state.value_arch;
    pipe::schedule_multicast(&mut runner.context(), &mut state, 999).unwrap();
    assert_eq!(state.value_arch, value_arch);
}

#[test]
fn test_empty_scheduler_retains_transmissions() {
    let mut runner = super::super::runner(Psa);
    let path = super::super::repo().join("p4spec/test/micro/sim-psa/psa.p4");
    let program = super::super::parse_program(runner.arena_mut(), &path);
    let mut state = psa::init_pipe(&mut runner.context(), program).unwrap();
    state.txs.push(Transmission {
        port: 9,
        packet: "AB".to_owned(),
    });
    let value_ctx = state.value_ctx;
    let value_arch = state.value_arch;
    scheduler::run_scheduler(&mut runner.context(), &mut state).unwrap();
    assert_eq!(state.value_ctx, value_ctx);
    assert_eq!(state.value_arch, value_arch);
    assert_eq!(state.txs.len(), 1);
    assert_eq!(state.txs[0].port, 9);
    assert_eq!(state.txs[0].packet, "AB");
}

fn counter_count(runner: &mut Runner, state: &SimState) -> i64 {
    let values = ["ip", "ig", "counter"]
        .into_iter()
        .map(|name| make::text(runner.arena_mut(), name.to_owned(), Span::default()).unwrap())
        .collect();
    let typ = typ::make::list(typ::make::var(
        p4spec_rust::phrase!(node: "id".to_owned(), span: Span::default()),
        vec![],
    ));
    let value_id =
        make::list(runner.arena_mut(), typ.node.into(), values, Span::default()).unwrap();
    let pipe::ObjectState::Counter(psa::object::Counter::Packets(counts)) =
        pipe::get_object_state(&mut runner.context(), state.value_arch, value_id).unwrap()
    else {
        panic!("packet counter")
    };
    unpack::signed_int(&counts[256]).unwrap()
}

#[test]
fn test_native_replication_fixture_order_and_persistent_counter() {
    let mut runner = super::super::runner(Psa);
    let path =
        super::super::repo().join("p4spec-rust/tests/fixtures/sim-plugin/psa/replication.p4");
    let program = super::super::parse_program(runner.arena_mut(), &path);
    let mut state = psa::init_pipe(&mut runner.context(), program).unwrap();
    let stmts = p4spec_rust::stf::parse::parse_file(path.with_extension("stf")).unwrap();
    let mut txs = Vec::new();
    let mut txs_expect = Vec::new();
    let mut counts = Vec::new();
    for stmt in stmts {
        match stmt.node {
            Statement::McGroupCreate { group_id: id_group } => {
                state.value_arch = pipe::mc_mgrp_create(
                    &mut runner.context(),
                    state.value_arch,
                    unpack::parse_signed_int(&id_group).unwrap(),
                )
                .unwrap()
            }
            Statement::McNodeCreate {
                replication_id: id_replication,
                ports,
            } => {
                let ports = ports
                    .iter()
                    .map(|port| unpack::parse_signed_int(port).unwrap())
                    .collect::<Vec<_>>();
                state.value_arch = pipe::mc_node_create(
                    &mut runner.context(),
                    state.value_arch,
                    unpack::parse_signed_int(&id_replication).unwrap(),
                    &ports,
                )
                .unwrap();
            }
            Statement::McNodeAssociate {
                group_id: id_group,
                handle,
            } => {
                state.value_arch = pipe::mc_node_associate(
                    &mut runner.context(),
                    state.value_arch,
                    unpack::parse_signed_int(&id_group).unwrap(),
                    unpack::parse_signed_int(&handle).unwrap(),
                )
                .unwrap()
            }
            Statement::MirroringAddMc {
                session,
                group_id: id_group,
            } => {
                state.value_arch = pipe::add_mirror_session_mc(
                    &mut runner.context(),
                    state.value_arch,
                    unpack::parse_signed_int(&session).unwrap(),
                    unpack::parse_signed_int(&id_group).unwrap(),
                )
                .unwrap()
            }
            Statement::Packet { port, packet } => {
                psa::drive_pipe(
                    &mut runner.context(),
                    &mut state,
                    &Transmission {
                        port: unpack::parse_signed_int(&port).unwrap(),
                        packet,
                    },
                )
                .unwrap();
                txs.extend(state.txs.iter().map(|tx| (tx.port, tx.packet.clone())));
                counts.push(counter_count(&mut runner, &state));
                let arch = pipe::get_arch_state(&mut runner.context(), state.value_arch).unwrap();
                assert!(arch.queue.is_empty());
                assert_eq!(arch.multicast.groups[&7], [0]);
                assert_eq!(arch.mirrortable[&5], 7);
            }
            Statement::Expect {
                port,
                packet_expected: Some(packet),
                ..
            } => txs_expect.push((unpack::parse_signed_int(&port).unwrap(), packet)),
            _ => {
                panic!("replication fixture contains only supported setup, packet and expectations")
            }
        }
    }
    assert_eq!(txs.len(), 12);
    assert_eq!(txs, txs_expect);
    assert_eq!(counts, [1, 3, 4, 5, 7]);
}

#[test]
fn test_stf_transformation_only_rewrites_register_names() {
    let stmts = p4spec_rust::stf::parse::parse_str("psa.stf", "register_read Ingress.reg 1\nregister_write myINGRESS.reg 2 3\nregister_reset ingress.reg\nsetdefault ingress.table action()\n").unwrap();
    let stmts = stmts
        .into_iter()
        .map(|stmt| psa::transform_stf_stmt(stmt.node))
        .collect::<Vec<_>>();
    assert!(
        matches!(&stmts[0], Statement::RegisterRead { name, .. } if name.as_str() == "ip.ig.reg")
    );
    assert!(
        matches!(&stmts[1], Statement::RegisterWrite { name, .. } if name.as_str() == "ip.ig.reg")
    );
    assert!(matches!(&stmts[2], Statement::RegisterReset { name } if name.as_str() == "ip.ig.reg"));
    assert!(
        matches!(&stmts[3], Statement::SetDefault { table, .. } if table.as_str() == "ingress.table")
    );
}
