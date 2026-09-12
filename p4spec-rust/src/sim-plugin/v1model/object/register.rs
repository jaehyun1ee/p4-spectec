use super::{ObjectResult, finish, repeat};
use crate::sim_plugin::spec_impl::{func, rel, unpack};
use crate::{
    lang::data::value::{Value, ValueArena},
    runner::{Extern, ExternError, Interface, Interpreter, RunnerContext},
};
use serde_derive_state::{DeserializeState, SerializeState};

#[derive(Clone, Debug, PartialEq, Eq, SerializeState, DeserializeState)]
#[serde(deny_unknown_fields, serialize_state = "ValueArena")]
#[serde(deserialize_state = "ValueArena")]
pub struct Register {
    #[serde(state)]
    pub value_typ: Value,
    #[serde(state)]
    pub values: Vec<Value>,
}

impl Register {
    /// A register object is created by calling its constructor.  This
    /// creates an array of 'size' identical elements, each with type
    /// T.  The array indices are in the range [0, size-1].  For
    /// example, this constructor call:
    ///
    /// ```text
    ///     register<bit<32>>(512) my_reg;
    ///
    /// ```
    /// allocates storage for 512 values, each with type bit<32>.
    ///
    /// register(bit<32> size);
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
        let [value_typ] = values_targ else {
            return Err(ExternError::Failure(format!(
                "register constructor expects 1 type argument, but {} were given",
                values_targ.len()
            ))
            .into());
        };
        let value_typ = *value_typ;
        let args = unpack::assoc_args(ctx.arena(), value_ids, value_args)?;
        let value_size = unpack::find_arg(&args, "size")?;
        let value_initial = func::default(ctx, value_typ)?;
        let size = unpack::signed_int(&unpack::p4_fixed_bit(ctx.arena(), &value_size)?.int)?;
        Ok(Self {
            value_typ,
            values: repeat(value_initial, size)?,
        })
    }

    /// read() reads the state of the register array stored at the
    /// specified index, and returns it as the value written to the
    /// result parameter.
    ///
    /// @param index The index of the register array element to be
    ///              read, normally a value in the range [0, size-1].
    /// @param result Only types T that are bit<W> are currently
    ///              supported.  When index is in range, the value of
    ///              result becomes the value read from the register
    ///              array element.  When index >= size, the final
    ///              value of result is not specified, and should be
    ///              ignored by the caller.
    ///
    /// void read(out T result, in bit<32> index);
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
        let value_ctx = rel::lvalue_write_var_local(ctx, value_ctx, value_arch, "result", value)?;
        Ok(finish(ctx.arena_mut(), self, value_ctx, value_arch, None)?)
    }
    /// write() writes the state of the register array at the specified
    /// index, with the value provided by the value parameter.
    ///
    /// If you wish to perform a read() followed later by a write() to
    /// the same register array element, and you wish the
    /// read-modify-write sequence to be atomic relative to other
    /// processed packets, then there may be parallel implementations
    /// of the v1model architecture for which you must execute them in
    /// a P4_16 block annotated with an @atomic annotation.  See the
    /// P4_16 language specification description of the @atomic
    /// annotation for more details.
    ///
    /// @param index The index of the register array element to be
    ///              written, normally a value in the range [0,
    ///              size-1].  If index >= size, no register state will
    ///              be updated.
    /// @param value Only types T that are bit<W> are currently
    ///              supported.  When index is in range, this
    ///              parameter's value is written into the register
    ///              array element specified by index.
    /// void write(in bit<32> index, in T value);
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
