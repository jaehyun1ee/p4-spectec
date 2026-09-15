use crate::lang::data::value::external::{DecodeContext, EncodeContext};
use crate::sim_plugin::spec::{args, func, unpack};
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
use num_traits::ToPrimitive;
use serde_derive_state::{DeserializeState, SerializeState};

#[derive(Clone, Debug, PartialEq, Eq, SerializeState, DeserializeState)]
#[serde(
    deny_unknown_fields,
    serialize_state = "EncodeContext<'arena>",
    ser_parameters = "'arena"
)]
#[serde(deserialize_state = "DecodeContext<'de>")]
pub struct Register {
    #[serde(state)]
    pub value_typ: Value,
    #[serde(state)]
    pub values: Vec<Value>,
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
        let args = args::assoc(ctx.arena(), value_ids, value_args)?;
        let value_size = args::find(&args, "size")?;
        let value_initial = match args.iter().find(|(name, _)| name == "initial_value") {
            Some((_, value)) => *value,
            None => func::default(ctx, value_typ)?,
        };
        let size = (unpack::p4_fixed_bit(ctx.arena(), &value_size)?.1)
            .to_i64()
            .ok_or_else(|| ExternError::Failure("integer outside i64 range".to_owned()))?
            as usize;
        Ok(Self {
            value_typ,
            values: vec![value_initial; size],
        })
    }

    /// `T read(in S index);`
    pub fn read<Interp, Iface, Exn>(
        self,
        ctx: &mut RunnerContext<'_, Interp, Iface, Exn>,
        value_ctx: Value,
        value_arch: Value,
    ) -> Result<(Self, Value, Value, Value), Interp::Error>
    where
        Iface: Interface,
        Exn: Extern,
        Interp: Interpreter<Iface, Exn>,
    {
        let value_idx = func::find_var_e_local(ctx, value_ctx, "index")?;
        let idx = (unpack::p4_fixed_bit(ctx.arena(), &value_idx)?.1)
            .to_i64()
            .ok_or_else(|| ExternError::Failure("integer outside i64 range".to_owned()))?;
        let idx = usize::try_from(idx)
            .map_err(|_| ExternError::Failure("negative register index".to_owned()))?;
        let value = match self.values.get(idx) {
            Some(value) => *value,
            None => func::default(ctx, self.value_typ)?,
        };
        let typ = typ::make::opt(typ::make::var(
            crate::phrase!(node: "value".to_owned(), span: Span::default()),
            Vec::new(),
        ));
        let value_opt = make::opt(
            ctx.arena_mut(),
            typ.node.into(),
            Some(value),
            Span::default(),
        )
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

    /// `void write(in S index, in T value);`
    pub fn write<Interp, Iface, Exn>(
        mut self,
        ctx: &mut RunnerContext<'_, Interp, Iface, Exn>,
        value_ctx: Value,
        value_arch: Value,
    ) -> Result<(Self, Value, Value, Value), Interp::Error>
    where
        Iface: Interface,
        Exn: Extern,
        Interp: Interpreter<Iface, Exn>,
    {
        let value_idx = func::find_var_e_local(ctx, value_ctx, "index")?;
        let idx = (unpack::p4_fixed_bit(ctx.arena(), &value_idx)?.1)
            .to_i64()
            .ok_or_else(|| ExternError::Failure("integer outside i64 range".to_owned()))?;
        let value_target = func::find_var_e_local(ctx, value_ctx, "value")?;
        if let Ok(idx) = usize::try_from(idx)
            && let Some(value) = self.values.get_mut(idx)
        {
            *value = value_target;
        }
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
