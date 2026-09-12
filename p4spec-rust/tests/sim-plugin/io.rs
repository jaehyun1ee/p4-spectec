use p4spec_rust::sim_plugin::io::{Expectation, Transmission, matches};

#[test]
fn test_packet_match_exact_prefix_and_wildcard() {
    let tx = Transmission {
        port: 1,
        packet: "ABCD".to_owned(),
    };
    let mut expect = Expectation {
        tx: Transmission {
            port: 1,
            packet: "A*".to_owned(),
        },
        exact: false,
    };
    assert!(matches(&tx, &expect));
    expect.exact = true;
    assert!(!matches(&tx, &expect));
    expect.tx.packet = "A*C*".to_owned();
    assert!(matches(&tx, &expect));
    expect.tx.packet = "a*C*".to_owned();
    assert!(!matches(&tx, &expect));
    expect.exact = false;
    expect.tx.packet = "a*".to_owned();
    assert!(!matches(&tx, &expect));
}

#[test]
fn test_packet_match_port_empty_and_short_output() {
    let mut tx = Transmission {
        port: 1,
        packet: "A".to_owned(),
    };
    let mut expect = Expectation {
        tx: Transmission {
            port: 1,
            packet: "AB".to_owned(),
        },
        exact: false,
    };
    assert!(!matches(&tx, &expect));
    expect.exact = true;
    assert!(!matches(&tx, &expect));
    expect.tx.packet.clear();
    assert!(!matches(&tx, &expect));
    expect.exact = false;
    assert!(matches(&tx, &expect));
    expect.tx.port = 2;
    assert!(!matches(&tx, &expect));
    expect.tx.port = 1;
    expect.exact = true;
    tx.packet.clear();
    assert!(matches(&tx, &expect));
    expect.tx.port = 2;
    assert!(!matches(&tx, &expect));
}
