use super::{ObjectResult, finish, repeat};
use crate::{
    lang::data::serialize::value as value_data,
    sim_plugin::spec_impl::{func, unpack},
    util::json::json,
};
use crate::{
    lang::data::value::{Value, ValueArena},
    runner::{Extern, ExternError, Interface, Interpreter, RunnerContext},
};
use serde::{Deserialize, Serialize};

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
        let value_size = unpack::find_arg(&args, "size")?;
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
