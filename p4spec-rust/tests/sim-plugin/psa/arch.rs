use p4spec_rust::{
    lang::{
        common::source::Span,
        data::value::{ValueArena, get, make},
    },
    sim_plugin::{
        core::object::PacketIn,
        psa::{
            arch::Arch,
            mirror,
            multicast::{Node, State},
            packet::{Entrypoint, Packet},
        },
    },
};
use serde_json::json;

#[test]
fn test_multicast_order_and_handle_wrap_roundtrip() {
    let mut state = State::default();
    state.group_create(7);
    state.node_create(42, &[12, 3, 12]);
    state.node_associate(7, 0);
    state.node_associate(7, 0);
    state.node_associate(8, 999);
    assert_eq!(state.groups[&7], [0, 0]);
    assert!(!state.groups.contains_key(&8));
    assert_eq!(
        state.nodes[&0],
        [
            Node {
                port: 12,
                instance: 42
            },
            Node {
                port: 3,
                instance: 42
            },
            Node {
                port: 12,
                instance: 42
            }
        ]
    );
    state.group_create(7);
    assert!(state.groups[&7].is_empty());
    state.handle_next = (1_i64 << 62) - 1;
    state.node_create(1, &[]);
    assert_eq!(state.handle_next, -(1_i64 << 62));
    let json = serde_json::to_value(&state).unwrap();
    assert_eq!(serde_json::from_value::<State>(json).unwrap(), state);
}

#[test]
fn test_mirror_table_native_roundtrip() {
    let table = mirror::Table::from([(-1, 1), (2, 22), (10, 10)]);
    let json = serde_json::to_value(&table).unwrap();
    assert_eq!(
        serde_json::from_value::<mirror::Table>(json).unwrap(),
        table
    );
}

#[test]
fn test_arch_queue_native_value_payload_roundtrip() {
    let mut arena = ValueArena::default();
    let value_ctx = make::text(
        &mut arena,
        "captured ingress context".to_owned(),
        Span::default(),
    )
    .unwrap();
    let mut arch = Arch::default();
    arch.queue.push_back(Packet {
        value_ctx,
        packet_in: PacketIn::init("aB01").unwrap().parse(4).unwrap().0,
        entrypoint: Entrypoint::Ingress,
    });
    let value_ctx = make::text(
        &mut arena,
        "captured egress context".to_owned(),
        Span::default(),
    )
    .unwrap();
    arch.queue.push_back(Packet {
        value_ctx,
        packet_in: PacketIn::init("Cd02").unwrap(),
        entrypoint: Entrypoint::Egress,
    });
    arch.mirrortable.insert(10, 5);
    arch.multicast.group_create(5);
    arch.multicast.node_create(42, &[12, 3, 12]);
    arch.multicast.node_associate(5, 0);
    let json = arch.to_json(&arena).unwrap();
    let mut arena_decoded = ValueArena::default();
    make::text(
        &mut arena_decoded,
        "unrelated arena value".to_owned(),
        Span::default(),
    )
    .unwrap();
    let mut arch_decoded = Arch::from_json(&mut arena_decoded, &json).unwrap();
    assert_eq!(arch_decoded.to_json(&arena_decoded).unwrap(), json);
    assert_eq!(arch_decoded.mirrortable, arch.mirrortable);
    assert_eq!(arch_decoded.multicast, arch.multicast);
    for (entrypoint, text, pkt_expect) in [
        (
            Entrypoint::Ingress,
            "captured ingress context",
            &arch.queue[0].packet_in,
        ),
        (
            Entrypoint::Egress,
            "captured egress context",
            &arch.queue[1].packet_in,
        ),
    ] {
        let pkt = arch_decoded.queue.pop_front().unwrap();
        assert_eq!(pkt.entrypoint, entrypoint);
        assert_eq!(get::text(&arena_decoded, &pkt.value_ctx).unwrap(), text);
        assert_eq!(&pkt.packet_in, pkt_expect);
        assert_eq!(
            pkt.packet_in.payload().unwrap(),
            pkt_expect.payload().unwrap()
        );
    }
    assert!(arch_decoded.queue.is_empty());
}

#[test]
fn test_arch_rejects_invalid_queued_packet_bounds() {
    let mut arena = ValueArena::default();
    let value_ctx = make::text(&mut arena, "context".to_owned(), Span::default()).unwrap();
    let mut arch = Arch::default();
    arch.queue.push_back(Packet {
        value_ctx,
        packet_in: PacketIn::init("AB01").unwrap(),
        entrypoint: Entrypoint::Ingress,
    });
    let mut json = arch.to_json(&arena).unwrap();
    json["queue"][0]["packet_in"]["idx"] = json!(17);
    assert!(Arch::from_json(&mut arena, &json).is_err());
    arch.queue[0].packet_in.idx = 17;
    assert!(arch.to_json(&arena).is_err());
}
