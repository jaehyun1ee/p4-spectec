//! The `counter` extern with indexed packet and byte counters
//!
//! An array of `size` counters, each counting packets, bytes, or both;
//! an out-of-range index leaves every counter unchanged.

use crate::sim_plugin::{
    core::object::PacketIn,
    spec::{args, func, unpack},
};
use crate::{
    lang::{
        common::source::Span,
        data::{
            typ,
            value::{Value, ValueArena, make},
        },
    },
    runner::{Extern, ExternError, Interface, Interpreter, RunnerContext},
};
use num_bigint::BigInt;
use num_traits::{One, Zero};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
/// Counter array by `CounterType`.
pub enum Counter {
    /// Packet counts.
    Packets(Vec<BigInt>),
    /// Byte counts.
    Bytes(Vec<BigInt>),
    /// Packet and byte counts.
    PacketsAndBytes(Vec<(BigInt, BigInt)>),
}

impl Counter {
    /// Creates `size` zeroed counters of the requested `CounterType`.
    ///
    /// Counters are updated by the program and read by the control plane:
    /// ```text
    /// counter(bit<32> size, CounterType type);
    /// ```
    pub fn init(
        arena: &ValueArena,
        _value_targs: Value,
        value_ids: Value,
        value_args: Value,
    ) -> Result<Self, ExternError> {
        let args = args::assoc(arena, value_ids, value_args)?;
        let value_size = args::find(&args, "size")?;
        let value_type = args::find(&args, "type")?;
        let size = usize::try_from(&unpack::p4_fixed_bit(arena, &value_size)?.1)?;
        let (id_enum, id_type) = unpack::p4_enum(arena, &value_type)?;
        // The type argument selects what is counted
        match (id_enum.as_str(), id_type.as_str()) {
            ("CounterType", "packets") => Ok(Self::Packets(vec![BigInt::zero(); size])),
            ("CounterType", "bytes") => Ok(Self::Bytes(vec![BigInt::zero(); size])),
            ("CounterType", "packets_and_bytes") => {
                Ok(Self::PacketsAndBytes(vec![(BigInt::zero(), BigInt::zero()); size]))
            }
            _ => Err(ExternError::Failure(format!(
                "invalid CounterType enum value: {id_enum}.{id_type}"
            ))),
        }
    }

    /// Adds one packet, the packet's bytes, or both to the counter at `index`.
    ///
    /// `index >= size` updates nothing:
    /// ```text
    /// void count(in bit<32> index);
    /// ```
    pub fn count<Interp, Iface, Ext>(
        mut self,
        ctx: &mut RunnerContext<'_, Interp, Iface, Ext>,
        value_ctx: Value,
        value_arch: Value,
        packet_in: &PacketIn,
    ) -> Result<(Self, Value, Value, Value), Interp::Error>
    where
        Iface: Interface,
        Ext: Extern,
        Interp: Interpreter<Iface, Ext>,
    {
        let value_idx = func::find_var_e_local(ctx, value_ctx, "index")?;
        let idx = usize::try_from(&unpack::p4_fixed_bit(ctx.arena(), &value_idx)?.1)
            .map_err(ExternError::from)?;
        // An out-of-range index leaves the array untouched
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
        // Return without a value
        let typ = typ::make::opt(typ::make::var(
            crate::phrase!(node: "value".to_owned(), span: Span::default()),
            Vec::new(),
        ));
        let value_opt = make::opt(ctx.arena_mut(), typ.node.into(), None, Span::default())
            .map_err(ExternError::from)?;
        let value_call_result = make::case_shaped! {
            arena: ctx.arena_mut(),
            shape: "RETURN value?",
            args: vec![value_opt],
            typ: "returnResult",
            span: Span::default(),
        }
        .map_err(ExternError::from)?;
        Ok((self, value_ctx, value_arch, value_call_result))
    }
}
