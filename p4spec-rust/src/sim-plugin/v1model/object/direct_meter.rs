//! The `direct_meter` extern, a meter attached to a table
//!
//! Metering is not modeled; `read()` always reports the color green.

use crate::sim_plugin::{
    core::object::PacketIn,
    spec::{args, func, pack, rel, unpack},
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
use num_traits::Zero;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
/// Direct meter by `MeterType`; the state is never updated.
pub enum DirectMeter {
    /// Meters packets regardless of size.
    Packets(BigInt),
    /// Meters bytes.
    Bytes(BigInt),
}

impl DirectMeter {
    /// Creates a direct meter of the requested `MeterType`.
    ///
    /// The object is attached to a table by its `meters` property.
    pub fn init(
        arena: &ValueArena,
        _value_targs: Value,
        value_ids: Value,
        value_args: Value,
    ) -> Result<Self, ExternError> {
        let args = args::assoc(arena, value_ids, value_args)?;
        let value_type = args::find(&args, "type")?;
        let (id_enum, id_type) = unpack::p4_enum(arena, &value_type)?;
        // The type argument selects what is metered
        match (id_enum.as_str(), id_type.as_str()) {
            ("MeterType", "packets") => Ok(Self::Packets(BigInt::zero())),
            ("MeterType", "bytes") => Ok(Self::Bytes(BigInt::zero())),
            _ => Err(ExternError::Failure(format!(
                "invalid CounterType enum value: {id_enum}.{id_type}"
            ))),
        }
    }

    /// Writes the meter color to `result`, always green (0).
    ///
    /// `T` must be `bit<W>` with `W >= 2`; 0 is green, 1 yellow, 2 red.
    pub fn read<Interp, Iface, Ext>(
        self,
        ctx: &mut RunnerContext<'_, Interp, Iface, Ext>,
        value_ctx: Value,
        value_arch: Value,
        _packet_in: &PacketIn,
    ) -> Result<(Self, Value, Value, Value), Interp::Error>
    where
        Iface: Interface,
        Ext: Extern,
        Interp: Interpreter<Iface, Ext>,
    {
        // The color width comes from the instantiated `T`
        let value_typ = func::find_type_e_local(ctx, value_ctx, "T")?;
        let value_typ = func::subst_type_e_local(ctx, value_ctx, value_typ)?;
        let size = func::sizeof_max_size_in_bits(ctx, value_typ)?;
        // Metering is not modeled: always GREEN
        let value = pack::p4_fixed_bit(ctx.arena_mut(), size, BigInt::zero())?;
        let value_ctx = rel::lvalue_write_var_local(ctx, value_ctx, value_arch, "result", value)?;
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
