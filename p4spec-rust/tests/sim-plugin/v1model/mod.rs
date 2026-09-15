#[path = "scheduler.rs"]
mod scheduler;

#[test]
fn test_clone_info_preserves_tuple_json() {
    use p4spec_rust::{
        lang::data::value::ValueArena,
        sim_plugin::{
            spec_impl::pack,
            v1model::packet::{CloneInfo, CloneType},
        },
    };

    let mut arena = ValueArena::new();
    let value_session = pack::p4_fixed_bit(&mut arena, 32.into(), 7.into()).unwrap();
    let value_idx = pack::p4_fixed_bit(&mut arena, 8.into(), 3.into()).unwrap();
    for (name, clone_type) in [("I2E", CloneType::I2E), ("E2E", CloneType::E2E)] {
        let value_clone_type = pack::p4_enum(&mut arena, "CloneType", name).unwrap();
        let info = CloneInfo::new(&arena, &value_clone_type, &value_session, &value_idx).unwrap();
        assert_eq!(info, CloneInfo(clone_type, 7, 3));
        let json = serde_json::json!([name, 7, 3]);
        assert_eq!(serde_json::to_value(info).unwrap(), json);
        assert_eq!(serde_json::from_value::<CloneInfo>(json).unwrap(), info);
    }
}

#[test]
fn test_native_v1model_micro_fixture() {
    use p4spec_rust::lang::data::value::external::Encoding;
    use p4spec_rust::{
        sim_plugin::{
            io::Rx,
            v1model::{self, V1Model},
        },
        stf::ast::Statement,
    };

    for encoding in [Encoding::ArenaRelative, Encoding::ArenaIndependent] {
        let mut runner = super::runner(V1Model::new(encoding));
        let path = super::repo().join("p4spec/test/micro/sim-v1model/v1model.p4");
        let program = super::parse_program(runner.arena_mut(), &path);
        let mut state = v1model::init_pipe(&mut runner.context(), program).unwrap();
        let stmts = p4spec_rust::stf::parse::parse_file(path.with_extension("stf")).unwrap();
        let mut txs = Vec::new();
        let mut txs_expect = Vec::new();
        for stmt in stmts {
            match stmt.node {
                Statement::Packet { port, packet } => {
                    v1model::drive_pipe(
                        &mut runner.context(),
                        &mut state,
                        &Rx {
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
                _ => panic!("micro fixture contains only packet/expect commands"),
            }
        }
        assert_eq!(txs, txs_expect);
        assert!(
            v1model::pipe::find_arch_state(&mut runner.context(), state.value_arch)
                .unwrap()
                .queue
                .is_empty()
        );
    }
}

#[test]
fn test_hash_adjust_range_boundaries() {
    use p4spec_rust::sim_plugin::v1model::func;

    assert_eq!(
        func::adjust(&5.into(), &12.into(), &20.into()).unwrap(),
        11.into()
    );
    assert_eq!(
        func::adjust(&5.into(), &0.into(), &20.into()).unwrap(),
        5.into()
    );
    assert!(func::adjust(&5.into(), &5.into(), &20.into()).is_err());
    assert!(func::adjust(&5.into(), &3.into(), &20.into()).is_err());
    assert_eq!(
        func::adjust(&5.into(), &12.into(), &(-20).into()).unwrap(),
        6.into()
    );
}

#[test]
fn test_multicast_handles_use_i64_range() {
    use p4spec_rust::sim_plugin::v1model::multicast::State;

    let mut state = State {
        handle_next: (1_i64 << 62) - 1,
        ..State::default()
    };
    state.node_create(1, &[]);
    assert_eq!(state.handle_next, 1_i64 << 62);
    state.handle_next = i64::MAX;
    state.node_create(1, &[]);
    assert_eq!(state.handle_next, i64::MIN);
}
