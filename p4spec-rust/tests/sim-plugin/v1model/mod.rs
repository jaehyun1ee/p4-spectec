#[path = "scheduler.rs"]
mod scheduler;

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
            v1model::pipe::get_arch_state(&mut runner.context(), state.value_arch)
                .unwrap()
                .queue
                .is_empty()
        );
    }
}
