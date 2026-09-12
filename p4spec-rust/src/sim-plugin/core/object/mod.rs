pub mod bits;
pub mod packet_in;
pub mod packet_out;

pub use bits::{
    bits_to_int_signed, bits_to_int_unsigned, bits_to_string, int_to_bits_signed,
    int_to_bits_unsigned, string_to_bits,
};
pub use packet_in::PacketIn;
pub use packet_out::PacketOut;

use crate::{runner::ExternError, sim_plugin::spec_impl::rel::CallResult};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PacketResult<Pkt> {
    pub pkt: Pkt,
    pub result: CallResult,
}

pub fn packet_to_string(pkt_in: &PacketIn, pkt_out: &PacketOut) -> Result<String, ExternError> {
    let bits: Vec<_> = pkt_out
        .bits
        .iter()
        .copied()
        .chain(pkt_in.payload()?.iter().copied())
        .collect();
    Ok(bits_to_string(&bits))
}
