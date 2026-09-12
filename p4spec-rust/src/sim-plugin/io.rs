//! Packet transmissions and STF expectations

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Transmission {
    pub port: i64,
    pub packet: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
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

impl std::fmt::Display for Transmission {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "({})", self.port)?;
        if !self.packet.is_empty() {
            write!(formatter, " {}", self.packet)?;
        }
        Ok(())
    }
}
