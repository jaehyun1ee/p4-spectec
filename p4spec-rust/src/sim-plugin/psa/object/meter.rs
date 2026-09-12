use super::{ObjectResult, finish, repeat};
use crate::sim_plugin::spec_impl::{pack, unpack};
use crate::{
    lang::data::value::{Value, ValueArena},
    runner::{Extern, ExternError, Interface, Interpreter, RunnerContext},
};
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Color {
    Red,
    Green,
    Yellow,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Meter {
    Packets(Vec<Color>),
    Bytes(Vec<Color>),
}

impl Meter {
    /// Indexed meter with `n_meters` independent meter states
    ///
    /// ```text
    /// extern Meter<S>
    /// Meter(bit<32> n_meters, PSA_MeterType_t type);
    /// ```
    pub fn init(
        arena: &ValueArena,
        _value_targs: Value,
        value_ids: Value,
        value_args: Value,
    ) -> Result<Self, ExternError> {
        let args = unpack::assoc_args(arena, value_ids, value_args)?;
        let value_size = unpack::find_arg(&args, "n_meters")?;
        let value_type = unpack::find_arg(&args, "type")?;
        let size = unpack::signed_int(&unpack::p4_fixed_bit(arena, &value_size)?.int)?;
        let (id_enum, id_type) = unpack::p4_enum(arena, &value_type)?;
        match (id_enum.as_str(), id_type.as_str()) {
            ("PSA_MeterType_t", "PACKETS") => Ok(Self::Packets(repeat(Color::Green, size)?)),
            ("PSA_MeterType_t", "BYTES") => Ok(Self::Bytes(repeat(Color::Green, size)?)),
            _ => Err(ExternError::Failure(format!(
                "invalid PSA_MeterType_t enum value: {id_enum}.{id_type}"
            ))),
        }
    }
    /// Perform a color aware meter update (see RFC 2698). The `color`
    /// parameter specifies the packet's color before the method call
    ///
    /// ```text
    /// PSA_MeterColor_t execute(in S index, in PSA_MeterColor_t color);
    /// ```
    pub fn execute_color_aware<Interp, Iface, Exn>(
        self,
        ctx: &mut RunnerContext<'_, Interp, Iface, Exn>,
        value_ctx: Value,
        value_arch: Value,
    ) -> Result<ObjectResult<Self>, Interp::Error>
    where
        Iface: Interface,
        Exn: Extern,
        Interp: Interpreter<Iface, Exn>,
    {
        // NOTE: returning GREEN for now
        let value_color = pack::p4_enum(ctx.arena_mut(), "PSA_MeterColor_t", "GREEN")?;
        Ok(finish(
            ctx.arena_mut(),
            self,
            value_ctx,
            value_arch,
            Some(value_color),
        )?)
    }
    /// Perform a color blind meter update (see RFC 2698). This may call
    /// `execute(index, MeterColor_t.GREEN)`, which has the same behavior
    ///
    /// `PSA_MeterColor_t execute(in S index);`
    pub fn execute_color_blind<Interp, Iface, Exn>(
        self,
        ctx: &mut RunnerContext<'_, Interp, Iface, Exn>,
        value_ctx: Value,
        value_arch: Value,
    ) -> Result<ObjectResult<Self>, Interp::Error>
    where
        Iface: Interface,
        Exn: Extern,
        Interp: Interpreter<Iface, Exn>,
    {
        // NOTE: returning GREEN for now
        self.execute_color_aware(ctx, value_ctx, value_arch)
    }
}
