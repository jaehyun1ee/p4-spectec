use super::{ObjectResult, finish, repeat};
use crate::sim_plugin::spec_impl::{func, unpack};
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
    /// Indirect counter with `n_counters` independent counter values, where
    /// every counter value has a data plane size specified by type `W`
    ///
    /// ```text
    /// extern Counter<W, S>
    /// Counter(bit<32> n_counters, PSA_CounterType_t type);
    /// ```
    pub fn init(
        arena: &ValueArena,
        _value_targs: Value,
        value_ids: Value,
        value_args: Value,
    ) -> Result<Self, ExternError> {
        let args = unpack::assoc_args(arena, value_ids, value_args)?;
        let value_size = unpack::find_arg(&args, "n_counters")?;
        let value_type = unpack::find_arg(&args, "type")?;
        let size = unpack::signed_int(&unpack::p4_fixed_bit(arena, &value_size)?.int)?;
        let (id_enum, id_type) = unpack::p4_enum(arena, &value_type)?;
        match (id_enum.as_str(), id_type.as_str()) {
            ("PSA_CounterType_t", "PACKETS") => Ok(Self::Packets(repeat(BigInt::zero(), size)?)),
            ("PSA_CounterType_t", "BYTES") => Ok(Self::Bytes(repeat(BigInt::zero(), size)?)),
            ("PSA_CounterType_t", "PACKETS_AND_BYTES") => Ok(Self::PacketsAndBytes(repeat(
                (BigInt::zero(), BigInt::zero()),
                size,
            )?)),
            _ => Err(ExternError::Failure(format!(
                "invalid PSA_CounterType_t enum value: {id_enum}.{id_type}"
            ))),
        }
    }

    /// `void count(in S index);`
    pub fn count<Interp, Iface, Exn>(
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
        let value_idx = func::find_var_e_local(ctx, value_ctx, "index")?;
        let idx = unpack::signed_int(&unpack::p4_fixed_bit(ctx.arena(), &value_idx)?.int)?;
        let Self::Packets(counts) = &mut self else {
            return Err(ExternError::Failure(
                "Only enum value PACKETS of PSA_CounterType_t is supported".to_owned(),
            )
            .into());
        };
        if let Ok(idx) = usize::try_from(idx)
            && let Some(count) = counts.get_mut(idx)
        {
            *count += BigInt::one();
        }
        Ok(finish(ctx.arena_mut(), self, value_ctx, value_arch, None)?)
    }
}
