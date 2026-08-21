use num_bigint::BigInt;
use p4spec_rust::{
    domain::source::Region,
    runtime::value::make,
    sim::psa::{Counter, MulticastState, Register, transform_stf},
    wire::sim_suite::StfStmt,
};

#[test]
fn psa_register_names_match_the_ocaml_transform() {
    let transformed = transform_stf(StfStmt::RegisterReset {
        name: "ingress.reg".to_owned(),
    });
    let StfStmt::RegisterReset { name } = transformed else {
        panic!("expected register reset");
    };
    assert_eq!(name, "ip.ig.reg");
}

#[test]
fn psa_counter_and_register_updates_are_bounded() {
    let mut counter = Counter::packets(3);
    counter.count(1).unwrap();
    counter.count(9).unwrap();
    assert_eq!(
        counter.packet_values().unwrap(),
        &[BigInt::from(0), BigInt::from(1), BigInt::from(0)]
    );

    let typ = make::text("T".to_owned(), Region::none());
    let initial = make::int(BigInt::from(4), Region::none());
    let replacement = make::int(BigInt::from(9), Region::none());
    let mut register = Register::new(typ, 2, initial.clone());
    register.write(1, replacement.clone());
    register.write(8, replacement);
    assert_eq!(register.read(0), Some(&initial));
    assert_eq!(register.values().len(), 2);
}

#[test]
fn psa_multicast_handles_follow_creation_order() {
    let mut state = MulticastState::default();
    state.create_group(100);
    assert_eq!(state.create_node(7, vec![1, 2]), 0);
    assert_eq!(state.create_node(8, vec![3]), 1);
    state.associate(100, 1);

    assert_eq!(state.replicas(100), vec![(3, 8)]);
}
