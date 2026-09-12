use super::{ObjectResult, finish};
use crate::sim_plugin::{
    hash,
    spec_impl::{func, pack, unpack},
};
use crate::{
    lang::data::value::{Value, ValueArena},
    runner::{Extern, ExternError, Interface, Interpreter, RunnerContext},
};
use num_bigint::BigInt;
use num_traits::Zero;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct HashExtern {
    pub algo: String,
}

impl HashExtern {
    /// ```text
    /// extern Hash<O>
    /// Hash(PSA_HashAlgorithm_t algo);
    /// ```
    pub fn init(
        arena: &ValueArena,
        _value_targs: Value,
        value_ids: Value,
        value_args: Value,
    ) -> Result<Self, ExternError> {
        let args = unpack::assoc_args(arena, value_ids, value_args)?;
        let value_algo = unpack::find_arg(&args, "algo")?;
        let (id_enum, id_type) = unpack::p4_enum(arena, &value_algo)?;
        if id_enum != "PSA_HashAlgorithm_t" {
            return Err(ExternError::Failure(
                "invalid PSA hash algorithm enum type".to_owned(),
            ));
        }
        let algo = match id_type.as_str() {
            "IDENTITY" => "identity",
            "CRC32" => "crc32",
            "CRC16" => "crc16",
            "ONES_COMPLEMENT16" => "csum16",
            algo => algo,
        }
        .to_owned();
        Ok(Self { algo })
    }
    /// Compute and return the hash for `data`
    ///
    /// `O get_hash<D>(in D data);`
    pub fn get_hash<Interp, Iface, Exn>(
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
        let value_data = func::find_var_e_local(ctx, value_ctx, "data")?;
        let values = unpack::p4_tuple(ctx.arena(), &value_data)?;
        let int_hash = hash::compute_checksum(&self.algo, None, ctx.arena(), &values)?;
        self.return_hash(ctx, value_ctx, value_arch, int_hash)
    }
    /// Compute the hash for `data`, reduce it modulo `max`, then add `base`
    ///
    /// `base` specifies the minimum return value. `max` is the hash modulus;
    /// an implementation may limit its largest supported value, for example
    /// to 32 or 256, and may only support powers of two. P4 developers should
    /// limit their choice to such values to maximize portability
    ///
    /// Returns `base + (h % max)`, where `h` is the hash value
    ///
    /// `O get_hash<T, D>(in T base, in D data, in T max);`
    pub fn get_hash_adjust<Interp, Iface, Exn>(
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
        let value_base = func::find_var_e_local(ctx, value_ctx, "base")?;
        let base = unpack::p4_fixed_bit(ctx.arena(), &value_base)?.int;
        let value_max = func::find_var_e_local(ctx, value_ctx, "max")?;
        let max = unpack::p4_fixed_bit(ctx.arena(), &value_max)?.int;
        let value_data = func::find_var_e_local(ctx, value_ctx, "data")?;
        let values = unpack::p4_tuple(ctx.arena(), &value_data)?;
        let int_hash = hash::compute_checksum(&self.algo, None, ctx.arena(), &values)?;
        if max <= BigInt::zero() {
            return Err(ExternError::Failure("hash modulus must be positive".to_owned()).into());
        }
        let int_hash = ((int_hash % &max) + &max) % &max + base;
        self.return_hash(ctx, value_ctx, value_arch, int_hash)
    }
    fn return_hash<Interp, Iface, Exn>(
        self,
        ctx: &mut RunnerContext<'_, Interp, Iface, Exn>,
        value_ctx: Value,
        value_arch: Value,
        int_hash: BigInt,
    ) -> Result<ObjectResult<Self>, Interp::Error>
    where
        Iface: Interface,
        Exn: Extern,
        Interp: Interpreter<Iface, Exn>,
    {
        let value_typ = func::find_type_e_local(ctx, value_ctx, "O")?;
        let value_result = pack::p4_arbitrary_int(ctx.arena_mut(), int_hash)?;
        let value_result = func::cast_op(ctx, value_typ, value_result)?;
        Ok(finish(
            ctx.arena_mut(),
            self,
            value_ctx,
            value_arch,
            Some(value_result),
        )?)
    }
}
