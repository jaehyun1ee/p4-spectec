use crate::sim_plugin::spec_impl::{func, unpack};
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
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct CounterArray {
    pub counts: Vec<i64>,
}

impl CounterArray {
    /// A counter array is a dense or sparse array of unsigned 32-bit values,
    /// visible to the control-plane as an EBPF map (array or hash).
    /// Each counter is addressed by a 32-bit index.
    /// Counters can only be incremented by the data-plane, but they can be read
    /// or reset by the control-plane.
    ///
    /// Allocate an array of counters.
    /// - `max_index`: Maximum counter index supported.
    /// - `sparse`: The counter array is supposed to be sparse.
    ///
    /// ```p4
    /// CounterArray(bit<32> max_index, bool sparse);
    /// ```
    pub fn init(
        arena: &ValueArena,
        value_ids: Value,
        value_args: Value,
    ) -> Result<Self, ExternError> {
        let args = unpack::assoc_args(arena, value_ids, value_args)?;
        let value_max = unpack::find_arg(&args, "max_index")?;
        let value_sparse = unpack::find_arg(&args, "sparse")?;
        let (_, int_max) = unpack::p4_fixed_bit(arena, &value_max)?;
        let idx_max = unpack::signed_int(&int_max)?;
        unpack::p4_bool(arena, &value_sparse)?;
        let len = usize::try_from(idx_max)
            .map_err(|_| ExternError::Failure("negative counter array size".to_owned()))?;
        let mut counts = Vec::new();
        counts
            .try_reserve_exact(len)
            .map_err(|error| ExternError::Failure(error.to_string()))?;
        counts.resize(len, 0);
        Ok(Self { counts })
    }

    /// Increment counter with specified index
    ///
    /// ```p4
    /// void increment(in bit<32> index);
    /// ```
    pub fn increment<Interp, Iface, Exn>(
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
        // Get "index"
        let value_idx = func::find_var_e_local(ctx, value_ctx, "index")?;
        let (_, int_idx) = unpack::p4_fixed_bit(ctx.arena(), &value_idx)?;
        let idx = unpack::signed_int(&int_idx)?;
        self.update(ctx, value_ctx, value_arch, idx, 1)
    }

    /// Add value to counter with specified index
    ///
    /// ```p4
    /// void add(in bit<32> index, in bit<32> value);
    /// ```
    pub fn add<Interp, Iface, Exn>(
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
        // Get "index"
        let value_idx = func::find_var_e_local(ctx, value_ctx, "index")?;
        let (_, int_idx) = unpack::p4_fixed_bit(ctx.arena(), &value_idx)?;
        let idx = unpack::signed_int(&int_idx)?;
        // Get "value"
        let value_add = func::find_var_e_local(ctx, value_ctx, "value")?;
        let (_, int_add) = unpack::p4_fixed_bit(ctx.arena(), &value_add)?;
        let int = unpack::signed_int(&int_add)?;
        self.update(ctx, value_ctx, value_arch, idx, int)
    }

    fn update<Interp, Iface, Exn>(
        mut self,
        ctx: &mut RunnerContext<'_, Interp, Iface, Exn>,
        value_ctx: Value,
        value_arch: Value,
        idx: i64,
        int: i64,
    ) -> Result<(Self, Value, Value, Value), Interp::Error>
    where
        Iface: Interface,
        Exn: Extern,
        Interp: Interpreter<Iface, Exn>,
    {
        // Update counter
        if let Ok(idx) = usize::try_from(idx)
            && let Some(count) = self.counts.get_mut(idx)
        {
            *count = count.wrapping_add(int).wrapping_shl(1) >> 1;
        }
        // Create call result
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
