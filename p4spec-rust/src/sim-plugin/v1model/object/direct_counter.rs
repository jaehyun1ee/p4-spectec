use super::{ObjectResult, finish};
use crate::sim_plugin::{core::object::PacketIn, spec_impl::unpack};
use crate::{
    lang::data::value::{Value, ValueArena},
    runner::{Extern, ExternError, Interface, Interpreter, RunnerContext},
};
use num_bigint::BigInt;
use num_traits::{One, Zero};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum DirectCounter {
    Packets(BigInt),
    Bytes(BigInt),
    PacketsAndBytes((BigInt, BigInt)),
}

impl DirectCounter {
    /// A direct_counter object is created by calling its constructor.
    /// You must provide a choice of whether to maintain only a packet
    /// count (CounterType.packets), only a byte count
    /// (CounterType.bytes), or both (CounterType.packets_and_bytes).
    /// After constructing the object, you can associate it with at
    /// most one table, by adding the following table property to the
    /// definition of that table:
    ///
    /// ```text
    ///     counters = <object_name>;
    ///
    /// ```
    /// Counters can be updated from your P4 program, but can only be
    /// read from the control plane.  If you need something that can be
    /// both read and written from the P4 program, consider using a
    /// register.
    ///
    /// direct_counter(CounterType type);
    pub fn init(
        arena: &ValueArena,
        _value_targs: Value,
        value_ids: Value,
        value_args: Value,
    ) -> Result<Self, ExternError> {
        let args = unpack::assoc_args(arena, value_ids, value_args)?;
        let value_type = unpack::find_arg(&args, "type")?;
        let (id_enum, id_type) = unpack::p4_enum(arena, &value_type)?;
        match (id_enum.as_str(), id_type.as_str()) {
            ("CounterType", "packets") => Ok(Self::Packets(BigInt::zero())),
            ("CounterType", "bytes") => Ok(Self::Bytes(BigInt::zero())),
            ("CounterType", "packets_and_bytes") => {
                Ok(Self::PacketsAndBytes((BigInt::zero(), BigInt::zero())))
            }
            _ => Err(ExternError::Failure(format!(
                "invalid CounterType enum value: {id_enum}.{id_type}"
            ))),
        }
    }

    /// The count() method is actually unnecessary in the v1model
    /// architecture.  This is because after a direct_counter object
    /// has been associated with a table as described in the
    /// documentation for the direct_counter constructor, every time
    /// the table is applied and a table entry is matched, the counter
    /// state associated with the matching entry is read, modified, and
    /// written back, atomically relative to the processing of other
    /// packets, regardless of whether the count() method is called in
    /// the body of that action.
    ///
    /// void count();
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
        match &mut self {
            Self::Packets(count) => *count += BigInt::one(),
            Self::Bytes(count) => *count += BigInt::from(packet_in.len),
            Self::PacketsAndBytes((count_packets, count_bytes)) => {
                *count_packets += BigInt::one();
                *count_bytes += BigInt::from(packet_in.len);
            }
        }
        Ok(finish(ctx.arena_mut(), self, value_ctx, value_arch, None)?)
    }
}
