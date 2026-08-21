use p4spec_rust::wire::sim_suite::{SimEntry, SimSuiteCodec, StfStmt};

const MINIMAL_SUITE: &[u8] =
    include_bytes!("../../p4spec/test/wire/minimal-sim-suite.expected.json");

#[test]
fn decodes_simulation_suite() {
    let suite = SimSuiteCodec::decode(MINIMAL_SUITE).expect("decode simulation suite");

    assert_eq!(suite.arch, "ebpf");
    assert_eq!(suite.entries.len(), 2);
    let SimEntry::Run { stf, .. } = &suite.entries[0] else {
        panic!("expected run entry");
    };
    assert_eq!(stf.len(), 17);
    assert!(matches!(stf[0], StfStmt::Wait));
    assert!(matches!(stf[2], StfStmt::Packet { .. }));
    assert!(matches!(stf[3], StfStmt::Expect { .. }));
    assert!(matches!(stf[5], StfStmt::Add { .. }));
    assert!(matches!(stf[16], StfStmt::RegisterReset { .. }));
    assert!(matches!(suite.entries[1], SimEntry::Exclude { .. }));
}

#[test]
fn rejects_unknown_simulation_architecture() {
    let input = String::from_utf8(MINIMAL_SUITE.to_vec())
        .expect("suite fixture is UTF-8")
        .replace("\"ebpf\"", "\"unknown\"");

    let error = SimSuiteCodec::decode(input.as_bytes()).expect_err("reject architecture");
    assert!(error.to_string().contains("unknown"));
}
