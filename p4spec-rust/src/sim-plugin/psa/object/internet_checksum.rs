use super::{ObjectResult, finish};
use crate::sim_plugin::{
    hash,
    spec_impl::{func, pack, unpack},
};
use crate::{
    lang::data::value::Value,
    runner::{Extern, Interface, Interpreter, RunnerContext},
};
use num_bigint::BigInt;
use num_traits::Zero;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct InternetChecksum {
    pub int: BigInt,
}

impl InternetChecksum {
    /// Checksum based on the `ONES_COMPLEMENT16` algorithm used in IPv4,
    /// TCP and UDP. Supports incremental updates through `subtract`
    /// (see IETF RFC 1624)
    ///
    /// ```text
    /// extern InternetChecksum
    /// InternetChecksum();
    /// ```
    pub fn init() -> Self {
        Self {
            int: BigInt::zero(),
        }
    }
    /// Reset internal state and prepare the unit for computation
    ///
    /// Every InternetChecksum instance is automatically initialized as if
    /// `clear()` had been called whenever the parser or control containing
    /// its instantiation executes. All maintained state is independent per
    /// packet
    ///
    /// `void clear();`
    pub fn clear<Interp, Iface, Exn>(
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
        Ok(finish(
            ctx.arena_mut(),
            Self::init(),
            value_ctx,
            value_arch,
            None,
        )?)
    }
    /// Add data to the checksum; `data` must be a multiple of 16 bits long
    ///
    /// `void add<T>(in T data);`
    pub fn add<Interp, Iface, Exn>(
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
        self.update(ctx, value_ctx, value_arch, "csum16")
    }
    /// Subtract data from the existing checksum; `data` must be a multiple
    /// of 16 bits long
    ///
    /// `void subtract<T>(in T data);`
    pub fn subtract<Interp, Iface, Exn>(
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
        self.update(ctx, value_ctx, value_arch, "csum16_sub")
    }
    fn update<Interp, Iface, Exn>(
        mut self,
        ctx: &mut RunnerContext<'_, Interp, Iface, Exn>,
        value_ctx: Value,
        value_arch: Value,
        algo: &str,
    ) -> Result<ObjectResult<Self>, Interp::Error>
    where
        Iface: Interface,
        Exn: Extern,
        Interp: Interpreter<Iface, Exn>,
    {
        let value_data = func::find_var_e_local(ctx, value_ctx, "data")?;
        let values = unpack::p4_tuple(ctx.arena(), &value_data)?;
        let int = hash::compute_checksum(algo, Some(&self.int), ctx.arena(), &values)?;
        self.int = hash::bitwise_neg(&int, &16.into())?;
        Ok(finish(ctx.arena_mut(), self, value_ctx, value_arch, None)?)
    }
    /// Get the checksum for data added and not removed since the last clear
    ///
    /// `bit<16> get();`
    pub fn get<Interp, Iface, Exn>(
        mut self,
        ctx: &mut RunnerContext<'_, Interp, Iface, Exn>,
        value_ctx: Value,
        value_arch: Value,
    ) -> Result<ObjectResult<Self>, Interp::Error>
    where
        Iface: Interface,
        Exn: Extern,
        Interp: Interpreter<Iface, Exn>,
    {
        self.int = hash::bitwise_neg(&self.int, &16.into())?;
        self.get_state(ctx, value_ctx, value_arch)
    }
    /// Get the current checksum computation state. The return value is only
    /// intended for a future call to `set_state`
    ///
    /// `bit<16> get_state();`
    pub fn get_state<Interp, Iface, Exn>(
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
        let value_checksum = pack::p4_fixed_bit(ctx.arena_mut(), 16.into(), self.int.clone())?;
        Ok(finish(
            ctx.arena_mut(),
            self,
            value_ctx,
            value_arch,
            Some(value_checksum),
        )?)
    }
    /// Restore state returned by an earlier `get_state` call on this
    /// InternetChecksum instance or a different one
    ///
    /// `void set_state(in bit<16> checksum_state);`
    pub fn set_state<Interp, Iface, Exn>(
        mut self,
        ctx: &mut RunnerContext<'_, Interp, Iface, Exn>,
        value_ctx: Value,
        value_arch: Value,
    ) -> Result<ObjectResult<Self>, Interp::Error>
    where
        Iface: Interface,
        Exn: Extern,
        Interp: Interpreter<Iface, Exn>,
    {
        let value_state = func::find_var_e_local(ctx, value_ctx, "checksum_state")?;
        self.int = unpack::p4_fixed_bit(ctx.arena(), &value_state)?.int;
        Ok(finish(ctx.arena_mut(), self, value_ctx, value_arch, None)?)
    }
}
