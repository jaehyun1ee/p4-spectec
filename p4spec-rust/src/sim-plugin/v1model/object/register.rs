//! The `register` extern, an array the program reads and writes
//!
//! Elements start at the element type's default value;
//! an out-of-range read yields that default,
//! an out-of-range write is ignored.

use crate::lang::data::value::external::{DecodeContext, EncodeContext};
use crate::sim_plugin::spec::{args, func, rel, unpack};
use crate::{
    lang::{
        common::source::Span,
        data::{
            typ,
            value::{Value, make},
        },
    },
    runner::{Extern, ExternError, Interface, Interpreter, RunnerContext},
};
use serde_derive_state::{DeserializeState, SerializeState};

#[derive(Clone, Debug, PartialEq, Eq, SerializeState, DeserializeState)]
#[serde(deny_unknown_fields, serialize_state = "EncodeContext<'arena>", ser_parameters = "'arena")]
#[serde(deserialize_state = "DecodeContext<'de>")]
/// Register array with its element type.
pub struct Register {
    #[serde(state)]
    /// Element type `T`.
    pub value_typ: Value,
    #[serde(state)]
    /// The `size` elements.
    pub values: Vec<Value>,
}

impl Register {
    /// Creates `size` elements of type `T`, each at `T`'s default value.
    ///
    /// For example, `register<bit<32>>(512) my_reg;`
    /// allocates 512 values of type `bit<32>`.
    pub fn init<Interp, Iface, Ext>(
        ctx: &mut RunnerContext<'_, Interp, Iface, Ext>,
        value_targs: Value,
        value_ids: Value,
        value_args: Value,
    ) -> Result<Self, Interp::Error>
    where
        Iface: Interface,
        Ext: Extern,
        Interp: Interpreter<Iface, Ext>,
    {
        let values_targ = crate::lang::data::value::get::list(ctx.arena(), &value_targs)
            .map_err(ExternError::from)?;
        // Exactly one type argument, the element type
        let [value_typ] = values_targ else {
            return Err(ExternError::Failure(format!(
                "register constructor expects 1 type argument, but {} were given",
                values_targ.len()
            ))
            .into());
        };
        let value_typ = *value_typ;
        let args = args::assoc(ctx.arena(), value_ids, value_args)?;
        let value_size = args::find(&args, "size")?;
        let value_initial = func::default(ctx, value_typ)?;
        let size = usize::try_from(&unpack::p4_fixed_bit(ctx.arena(), &value_size)?.1)
            .map_err(ExternError::from)?;
        Ok(Self { value_typ, values: vec![value_initial; size] })
    }

    /// Writes the element at `index` to `result`.
    ///
    /// Only `bit<W>` element types are supported by `v1model.p4`;
    /// an out-of-range index yields the element type's default.
    pub fn read<Interp, Iface, Ext>(
        self,
        ctx: &mut RunnerContext<'_, Interp, Iface, Ext>,
        value_ctx: Value,
        value_arch: Value,
    ) -> Result<(Self, Value, Value, Value), Interp::Error>
    where
        Iface: Interface,
        Ext: Extern,
        Interp: Interpreter<Iface, Ext>,
    {
        let value_idx = func::find_var_e_local(ctx, value_ctx, "index")?;
        let idx = usize::try_from(&unpack::p4_fixed_bit(ctx.arena(), &value_idx)?.1)
            .map_err(ExternError::from)?;
        // Out of range: the default, since the result is unspecified
        let value = match self.values.get(idx) {
            Some(value) => *value,
            None => func::default(ctx, self.value_typ)?,
        };
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

    /// Stores `value` at `index`.
    ///
    /// An out-of-range index changes nothing;
    /// atomicity of a read-modify-write is the program's concern via `@atomic`.
    pub fn write<Interp, Iface, Ext>(
        mut self,
        ctx: &mut RunnerContext<'_, Interp, Iface, Ext>,
        value_ctx: Value,
        value_arch: Value,
    ) -> Result<(Self, Value, Value, Value), Interp::Error>
    where
        Iface: Interface,
        Ext: Extern,
        Interp: Interpreter<Iface, Ext>,
    {
        let value_idx = func::find_var_e_local(ctx, value_ctx, "index")?;
        let idx = usize::try_from(&unpack::p4_fixed_bit(ctx.arena(), &value_idx)?.1)
            .map_err(ExternError::from)?;
        let value_target = func::find_var_e_local(ctx, value_ctx, "value")?;
        // Out of range: ignored
        if let Some(value) = self.values.get_mut(idx) {
            *value = value_target;
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
