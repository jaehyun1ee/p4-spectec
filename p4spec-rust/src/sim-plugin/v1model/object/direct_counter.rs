//! The `direct_counter` extern, a counter attached to a table
//!
//! The simulator keeps one count per object,
//! bumped only when `count()` is called.

use crate::sim_plugin::{
    core::object::PacketIn,
    spec::{args, unpack},
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
/// Direct counter by `CounterType`.
pub enum DirectCounter {
    /// Packet count.
    Packets(BigInt),
    /// Byte count.
    Bytes(BigInt),
    /// Packet and byte counts.
    PacketsAndBytes((BigInt, BigInt)),
}

impl DirectCounter {
    /// Creates a zeroed direct counter of the requested `CounterType`.
    ///
    /// The object is attached to a table by its `counters` property:
    /// ```text
    /// direct_counter(CounterType type);
    /// ```
    pub fn init(
        arena: &ValueArena,
        _value_targs: Value,
        value_ids: Value,
        value_args: Value,
    ) -> Result<Self, ExternError> {
        let args = args::assoc(arena, value_ids, value_args)?;
        let value_type = args::find(&args, "type")?;
        let (id_enum, id_type) = unpack::p4_enum(arena, &value_type)?;
        // The type argument selects what is counted
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

    /// Adds one packet, the packet's bytes, or both to the counter.
    ///
    /// `v1model.p4` counts on every table match;
    /// here only an explicit call counts:
    /// ```text
    /// void count();
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
        match &mut self {
            Self::Packets(count) => *count += BigInt::one(),
            Self::Bytes(count) => *count += BigInt::from(packet_in.len),
            Self::PacketsAndBytes((count_packets, count_bytes)) => {
                *count_packets += BigInt::one();
                *count_bytes += BigInt::from(packet_in.len);
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
