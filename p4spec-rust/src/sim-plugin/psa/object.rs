use num_bigint::BigInt;
use num_traits::{One, Zero};
use serde::{Deserialize, Serialize};

use crate::{
    lang::data::{
        serialize::value as value_data,
        value::{Value, ValueArena},
    },
    runner::{Extern, ExternError, Interface, Interpreter, RunnerContext},
    util::json::json,
};

use super::super::{
    hash,
    spec_impl::{func, pack, rel::CallResult, unpack},
};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ObjectResult<Object> {
    pub object: Object,
    pub result: CallResult,
}

fn finish<Object>(
    arena: &mut ValueArena,
    object: Object,
    value_ctx: Value,
    value_arch: Value,
    value: Option<Value>,
) -> Result<ObjectResult<Object>, ExternError> {
    let value_call_result = pack::return_result(arena, value)?;
    Ok(ObjectResult {
        object,
        result: CallResult {
            value_ctx,
            value_arch,
            value_call_result,
        },
    })
}

fn find_arg(args: &[unpack::ArgumentValue], name: &str) -> Result<Value, ExternError> {
    args.iter()
        .find(|arg| arg.name == name)
        .map(|arg| arg.value)
        .ok_or_else(|| ExternError::Failure(format!("argument not found: {name}")))
}

fn repeat<Value: Clone>(value: Value, len: i64) -> Result<Vec<Value>, ExternError> {
    let len = usize::try_from(len)
        .map_err(|_| ExternError::Failure("negative object size".to_owned()))?;
    let mut values = Vec::new();
    values
        .try_reserve_exact(len)
        .map_err(|error| ExternError::Failure(error.to_string()))?;
    values.resize(len, value);
    Ok(values)
}

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
        let value_size = find_arg(&args, "n_counters")?;
        let value_type = find_arg(&args, "type")?;
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
        let value_size = find_arg(&args, "n_meters")?;
        let value_type = find_arg(&args, "type")?;
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

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Register {
    pub value_typ: Value,
    pub values: Vec<Value>,
}

// Arena values are encoded structurally before serializing register state
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct RegisterJson {
    json_typ: json,
    jsons: Vec<json>,
}

impl Register {
    /// Instantiate an array of `size` registers with undefined initial values,
    /// or initialize every register to the supplied `initial_value`
    ///
    /// ```text
    /// extern Register<T, S>
    /// Register(bit<32> size);
    /// Register(bit<32> size, T initial_value);
    /// ```
    pub fn init<Interp, Iface, Exn>(
        ctx: &mut RunnerContext<'_, Interp, Iface, Exn>,
        value_targs: Value,
        value_ids: Value,
        value_args: Value,
    ) -> Result<Self, Interp::Error>
    where
        Iface: Interface,
        Exn: Extern,
        Interp: Interpreter<Iface, Exn>,
    {
        let values_targ = crate::lang::data::value::get::list(ctx.arena(), &value_targs)
            .map_err(ExternError::from)?;
        let [value_typ, _] = values_targ else {
            return Err(ExternError::Failure(format!(
                "Register constructor expects 2 type arguments, but {} were given",
                values_targ.len()
            ))
            .into());
        };
        let value_typ = *value_typ;
        let args = unpack::assoc_args(ctx.arena(), value_ids, value_args)?;
        let value_size = find_arg(&args, "size")?;
        let value_initial = match args.iter().find(|arg| arg.name == "initial_value") {
            Some(arg) => arg.value,
            None => func::default(ctx, value_typ)?,
        };
        let size = unpack::signed_int(&unpack::p4_fixed_bit(ctx.arena(), &value_size)?.int)?;
        Ok(Self {
            value_typ,
            values: repeat(value_initial, size)?,
        })
    }
    pub fn to_json(&self, arena: &ValueArena) -> Result<json, ExternError> {
        let reg = RegisterJson {
            json_typ: value_data::encode(arena, &self.value_typ),
            jsons: self
                .values
                .iter()
                .map(|value| value_data::encode(arena, value))
                .collect(),
        };
        serde_json::to_value(reg).map_err(|error| ExternError::Failure(error.to_string()))
    }

    pub fn from_json(arena: &mut ValueArena, json: &json) -> Result<Self, ExternError> {
        let reg: RegisterJson = serde_json::from_value(json.clone())
            .map_err(|error| ExternError::Failure(error.to_string()))?;
        let value_typ = value_data::decode(arena, &reg.json_typ).map_err(ExternError::from)?;
        let values = reg
            .jsons
            .iter()
            .map(|json| value_data::decode(arena, json).map_err(ExternError::from))
            .collect::<Result<Vec<_>, _>>()?;
        Ok(Self { value_typ, values })
    }

    /// `T read(in S index);`
    pub fn read<Interp, Iface, Exn>(
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
        let value_idx = func::find_var_e_local(ctx, value_ctx, "index")?;
        let idx = unpack::signed_int(&unpack::p4_fixed_bit(ctx.arena(), &value_idx)?.int)?;
        let idx = usize::try_from(idx)
            .map_err(|_| ExternError::Failure("negative register index".to_owned()))?;
        let value = match self.values.get(idx) {
            Some(value) => *value,
            None => func::default(ctx, self.value_typ)?,
        };
        Ok(finish(
            ctx.arena_mut(),
            self,
            value_ctx,
            value_arch,
            Some(value),
        )?)
    }
    /// `void write(in S index, in T value);`
    pub fn write<Interp, Iface, Exn>(
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
        let value_target = func::find_var_e_local(ctx, value_ctx, "value")?;
        if let Ok(idx) = usize::try_from(idx)
            && let Some(value) = self.values.get_mut(idx)
        {
            *value = value_target;
        }
        Ok(finish(ctx.arena_mut(), self, value_ctx, value_arch, None)?)
    }
}

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
        let value_algo = find_arg(&args, "algo")?;
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
