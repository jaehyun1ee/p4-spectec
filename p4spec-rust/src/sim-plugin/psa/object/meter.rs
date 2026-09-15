use crate::sim_plugin::spec::{args, pack, unpack};
use crate::{
    lang::{
        common::source::Span,
        data::{
            typ,
            value::{Value, ValueArena, make},
        },
    },
    runner::{Extern, ExternError, Interface, Interpreter, RunnerContext},
};
use num_traits::ToPrimitive;
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
        let args = args::assoc(arena, value_ids, value_args)?;
        let value_size = args::find(&args, "n_meters")?;
        let value_type = args::find(&args, "type")?;
        let size = (unpack::p4_fixed_bit(arena, &value_size)?.1)
            .to_i64()
            .ok_or_else(|| ExternError::Failure("integer outside i64 range".to_owned()))?
            as usize;
        let (id_enum, id_type) = unpack::p4_enum(arena, &value_type)?;
        match (id_enum.as_str(), id_type.as_str()) {
            ("PSA_MeterType_t", "PACKETS") => Ok(Self::Packets(vec![Color::Green; size])),
            ("PSA_MeterType_t", "BYTES") => Ok(Self::Bytes(vec![Color::Green; size])),
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
    ) -> Result<(Self, Value, Value, Value), Interp::Error>
    where
        Iface: Interface,
        Exn: Extern,
        Interp: Interpreter<Iface, Exn>,
    {
        // NOTE: returning GREEN for now
        let value_color = pack::p4_enum(ctx.arena_mut(), "PSA_MeterColor_t", "GREEN")?;
        let typ = typ::make::opt(typ::make::var(
            crate::phrase!(node: "value".to_owned(), span: Span::default()),
            Vec::new(),
        ));
        let value_opt = make::opt(
            ctx.arena_mut(),
            typ.node.into(),
            Some(value_color),
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

    /// Perform a color blind meter update (see RFC 2698). This may call
    /// `execute(index, MeterColor_t.GREEN)`, which has the same behavior
    ///
    /// `PSA_MeterColor_t execute(in S index);`
    pub fn execute_color_blind<Interp, Iface, Exn>(
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
        // NOTE: returning GREEN for now
        self.execute_color_aware(ctx, value_ctx, value_arch)
    }
}
