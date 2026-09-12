#[path = "scheduler.rs"]
mod scheduler;

#[test]
fn test_native_v1model_micro_fixture() {
    use p4spec_rust::{
        sim_plugin::{
            io::Transmission,
            v1model::{self, V1Model},
        },
        stf::ast::Statement,
    };
    let mut runner = super::runner(V1Model);
    let path = super::repo().join("p4spec/test/micro/sim-v1model/v1model.p4");
    let program = super::parse_program(runner.arena_mut(), &path);
    let mut state = v1model::init_pipe(&mut runner.context(), program).unwrap();
    let stmts = p4spec_rust::stf::parse::parse_file(path.with_extension("stf")).unwrap();
    let mut expected = Vec::new();
    for stmt in stmts {
        match stmt.node {
            Statement::Packet { port, packet } => v1model::drive_pipe(
                &mut runner.context(),
                &mut state,
                &Transmission {
                    port: port.parse().unwrap(),
                    packet,
                },
            )
            .unwrap(),
            Statement::Expect {
                port,
                packet_expected: Some(packet),
                ..
            } => expected.push((port.parse::<i64>().unwrap(), packet.to_ascii_uppercase())),
            _ => panic!("micro fixture contains only packet/expect commands"),
        }
    }
    assert_eq!(
        state
            .txs
            .iter()
            .map(|tx| (tx.port, tx.packet.clone()))
            .collect::<Vec<_>>(),
        expected
    );
    assert!(
        v1model::pipe::get_arch_state(&mut runner.context(), state.value_arch)
            .unwrap()
            .queue
            .is_empty()
    );
}
