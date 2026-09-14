use crate::runner::ExternError;

use super::{PacketIn, PacketOut, bits::bits_to_string};

pub fn to_string(pkt_in: &PacketIn, pkt_out: &PacketOut) -> Result<String, ExternError> {
    let bits: Vec<_> = pkt_out
        .bits
        .iter()
        .copied()
        .chain(pkt_in.payload()?.iter().copied())
        .collect();
    Ok(bits_to_string(&bits))
}
