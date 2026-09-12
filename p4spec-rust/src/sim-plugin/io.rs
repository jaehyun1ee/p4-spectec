//! Packet transmissions and STF expectations

pub struct Transmission {
    pub port: i64,
    pub packet: String,
}

pub struct Expectation {
    pub tx: Transmission,
    pub exact: bool,
}

/// Compares ASCII packets case-sensitively, with `*` matching one nibble
pub fn matches(tx: &Transmission, expect: &Expectation) -> bool {
    if tx.port != expect.tx.port || (expect.exact && tx.packet.len() != expect.tx.packet.len()) {
        return false;
    }
    tx.packet
        .get(..expect.tx.packet.len())
        .is_some_and(|packet| crate::stf::r#match::matches(packet, &expect.tx.packet))
}
