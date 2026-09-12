use super::PacketResult;
use crate::{
    lang::data::value::{Value, get},
    runner::{Extern, ExternError, Interface, Interpreter, RunnerContext},
    sim_plugin::spec_impl::{func, pack, rel::CallResult},
};
use serde::{Deserialize, Serialize};

/// Output packet data accumulated by emission
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PacketOut {
    pub bits: Vec<bool>,
}

impl PacketOut {
    /// Appends the header's bits to the output packet
    ///
    /// ```text
    /// void emit<T>(in T hdr);
    /// ```
    pub fn emit<Interp, Iface, Exn>(
        &self,
        ctx: &mut RunnerContext<'_, Interp, Iface, Exn>,
        value_ctx: Value,
        value_arch: Value,
    ) -> Result<PacketResult<Self>, Interp::Error>
    where
        Iface: Interface,
        Exn: Extern,
        Interp: Interpreter<Iface, Exn>,
    {
        let value_hdr = func::find_var_e_local(ctx, value_ctx, "hdr")?;
        let value_bits = func::write_bits_from_value(ctx, value_hdr)?;
        let bits = get::list(ctx.arena(), &value_bits)
            .map_err(ExternError::from)?
            .iter()
            .map(|value| get::bool(ctx.arena(), value))
            .collect::<Result<Vec<_>, _>>()
            .map_err(ExternError::from)?;
        let pkt = Self {
            bits: self.bits.iter().copied().chain(bits).collect(),
        };
        let value_call_result = pack::return_result(ctx.arena_mut(), None)?;
        Ok(PacketResult {
            pkt,
            result: CallResult {
                value_ctx,
                value_arch,
                value_call_result,
            },
        })
    }
}
