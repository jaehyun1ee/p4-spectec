use super::{ObjectResult, finish, repeat};
use crate::sim_plugin::{
    core::object::PacketIn,
    spec_impl::{func, unpack},
};
use crate::{
    lang::data::value::{Value, ValueArena},
    runner::{Extern, ExternError, Interface, Interpreter, RunnerContext},
};
use num_bigint::BigInt;
use num_traits::{One, Zero};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Counter {
    Packets(Vec<BigInt>),
    Bytes(Vec<BigInt>),
    PacketsAndBytes(Vec<(BigInt, BigInt)>),
}

impl Counter {
    /// A counter object is created by calling its constructor.  This
    /// creates an array of counter states, with the number of counter
    /// states specified by the size parameter.  The array indices are
    /// in the range [0, size-1].
    ///
    /// You must provide a choice of whether to maintain only a packet
    /// count (CounterType.packets), only a byte count
    /// (CounterType.bytes), or both (CounterType.packets_and_bytes).
    ///
    /// Counters can be updated from your P4 program, but can only be
    /// read from the control plane.  If you need something that can be
    /// both read and written from the P4 program, consider using a
    /// register.
    ///
    /// counter(bit<32> size, CounterType type);
    pub fn init(
        arena: &ValueArena,
        _value_targs: Value,
        value_ids: Value,
        value_args: Value,
    ) -> Result<Self, ExternError> {
        let args = unpack::assoc_args(arena, value_ids, value_args)?;
        let value_size = unpack::find_arg(&args, "size")?;
        let value_type = unpack::find_arg(&args, "type")?;
        let size = unpack::signed_int(&unpack::p4_fixed_bit(arena, &value_size)?.int)?;
        let (id_enum, id_type) = unpack::p4_enum(arena, &value_type)?;
        match (id_enum.as_str(), id_type.as_str()) {
            ("CounterType", "packets") => Ok(Self::Packets(repeat(BigInt::zero(), size)?)),
            ("CounterType", "bytes") => Ok(Self::Bytes(repeat(BigInt::zero(), size)?)),
            ("CounterType", "packets_and_bytes") => Ok(Self::PacketsAndBytes(repeat(
                (BigInt::zero(), BigInt::zero()),
                size,
            )?)),
            _ => Err(ExternError::Failure(format!(
                "invalid CounterType enum value: {id_enum}.{id_type}"
            ))),
        }
    }

    /// count() causes the counter state with the specified index to be
    ///  read, modified, and written back, atomically relative to the
    ///  processing of other packets, updating the packet count, byte
    ///  count, or both, depending upon the CounterType of the counter
    ///  instance used when it was constructed.
    ///
    ///  @param index The index of the counter state in the array to be
    ///               updated, normally a value in the range [0,
    ///               size-1].  If index >= size, no counter state will be
    ///               updated.
    ///
    /// void count(in bit<32> index);
    pub fn count<Interp, Iface, Exn>(
        mut self,
        ctx: &mut RunnerContext<'_, Interp, Iface, Exn>,
        value_ctx: Value,
        value_arch: Value,
        packet_in: &PacketIn,
    ) -> Result<ObjectResult<Self>, Interp::Error>
    where
        Iface: Interface,
        Exn: Extern,
        Interp: Interpreter<Iface, Exn>,
    {
        let value_idx = func::find_var_e_local(ctx, value_ctx, "index")?;
        let idx = unpack::signed_int(&unpack::p4_fixed_bit(ctx.arena(), &value_idx)?.int)?;
        if let Ok(idx) = usize::try_from(idx) {
            match &mut self {
                Self::Packets(counts) => {
                    if let Some(count) = counts.get_mut(idx) {
                        *count += BigInt::one();
                    }
                }
                Self::Bytes(counts) => {
                    if let Some(count) = counts.get_mut(idx) {
                        *count += BigInt::from(packet_in.len);
                    }
                }
                Self::PacketsAndBytes(counts) => {
                    if let Some((count_packets, count_bytes)) = counts.get_mut(idx) {
                        *count_packets += BigInt::one();
                        *count_bytes += BigInt::from(packet_in.len);
                    }
                }
            }
        }
        Ok(finish(ctx.arena_mut(), self, value_ctx, value_arch, None)?)
    }
}
