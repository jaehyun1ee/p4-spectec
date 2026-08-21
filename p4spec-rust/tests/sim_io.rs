use p4spec_rust::sim::io::{Expectation, PacketIo, Tx, compare_packet};

#[test]
fn wildcard_and_prefix_expectations_match_ocaml() {
    assert!(compare_packet(false, "A1B2C3", "A1**"));
    assert!(compare_packet(true, "A1B2", "A1**"));
    assert!(!compare_packet(true, "A1B2C3", "A1**"));
    assert!(!compare_packet(false, "A1", "A1**"));

    let mut io = PacketIo::default();
    io.push_expectation(Expectation::new(1, "A1**", true))
        .unwrap();
    assert_eq!(
        io.push_output(Tx::new(1, "A1B2")).unwrap(),
        Some(Tx::new(1, "A1**"))
    );

    let mut io = PacketIo::default();
    io.push_output(Tx::new(1, "A1B2")).unwrap();
    assert_eq!(
        io.push_expectation(Expectation::new(1, "A1**", true))
            .unwrap(),
        Some(Tx::new(1, "A1B2"))
    );
}

#[test]
fn output_first_matches_by_port_and_preserves_other_ports() {
    let mut io = PacketIo::default();
    io.push_output(Tx::new(2, "22")).unwrap();
    io.push_output(Tx::new(1, "11")).unwrap();

    let matched = io
        .push_expectation(Expectation::new(1, "11", true))
        .unwrap();

    assert_eq!(matched, Some(Tx::new(1, "11")));
    assert_eq!(io.outputs(), &[Tx::new(2, "22")]);
}

#[test]
fn expectation_first_matches_by_port_and_preserves_other_ports() {
    let mut io = PacketIo::default();
    io.push_expectation(Expectation::new(2, "22", true))
        .unwrap();
    io.push_expectation(Expectation::new(1, "11", true))
        .unwrap();

    let matched = io.push_output(Tx::new(1, "11")).unwrap();

    assert_eq!(matched, Some(Tx::new(1, "11")));
    assert_eq!(io.expectations(), &[Expectation::new(2, "22", true)]);
}

#[test]
fn same_port_mismatch_is_immediate() {
    let mut io = PacketIo::default();
    io.push_expectation(Expectation::new(1, "AA", true))
        .unwrap();

    let error = io.push_output(Tx::new(1, "BB")).unwrap_err();

    assert!(error.to_string().contains("expected (1) AA but got (1) BB"));
}

#[test]
fn finish_reports_remaining_outputs_and_expectations() {
    let mut io = PacketIo::default();
    io.push_output(Tx::new(1, "AA")).unwrap();
    io.push_expectation(Expectation::new(2, "BB", true))
        .unwrap();

    let error = io.finish().unwrap_err();
    let message = error.to_string();
    assert!(message.contains("[FAIL] Remaining packets to be matched:\n(1) AA"));
    assert!(message.contains("[FAIL] Expected packets to be output:\n(2) BB"));
}
