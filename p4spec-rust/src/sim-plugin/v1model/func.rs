//! v1model extern functions
//!
//! Doc comments summarize the extern's description in `v1model.p4`.
//! random, clone, truncate, assert, and assume are not implemented;
//! the pipeline's dispatch rejects them as unsupported calls.

use super::{V1Model, packet::CloneInfo, pipe};
use crate::sim_plugin::{
    core::object::PacketIn,
    hash as checksum,
    spec::{func, pack, rel, unpack},
};
use crate::{
    lang::{
        common::source::Span,
        data::{
            typ,
            value::{Value, ValueArena, make},
        },
    },
    runner::{Extern, ExternError, Interface, Interpreter, RunnerContext},
    util::bigint::remainder,
};
use num_bigint::BigInt;
use num_traits::Zero;

/// Sends `data` to the control plane; a no-op here.
///
/// Only supported in the ingress control;
/// the BMv2 implementation ignores `receiver` as well.
pub fn digest<Interp, Iface, Ext>(
    ctx: &mut RunnerContext<'_, Interp, Iface, Ext>,
    value_ctx: Value,
    value_arch: Value,
) -> Result<(Value, Value, Value), Interp::Error>
where
    Iface: Interface,
    Ext: Extern,
    Interp: Interpreter<Iface, Ext>,
{
    // No-op in the source simulator
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
    Ok((value_ctx, value_arch, value_call_result))
}

/// Marks the packet for dropping at the end of ingress or egress.
///
/// Sets `standard_metadata.egress_spec` to the drop port and clears
/// `standard_metadata.mcast_grp`; later code may still change either.
pub fn mark_to_drop<Interp, Iface, Ext>(
    ctx: &mut RunnerContext<'_, Interp, Iface, Ext>,
    value_ctx: Value,
    value_arch: Value,
) -> Result<(Value, Value, Value), Interp::Error>
where
    Iface: Interface,
    Ext: Extern,
    Interp: Interpreter<Iface, Ext>,
{
    // The drop port is 511 on 9 bits
    let value_egress_spec = pack::p4_fixed_bit(ctx.arena_mut(), 9.into(), 511.into())?;
    let value_ctx = rel::lvalue_write_dot_local(
        ctx,
        value_ctx,
        value_arch,
        "standard_metadata",
        "egress_spec",
        value_egress_spec,
    )?;
    // No multicast for a dropped packet
    let value_mcast_grp = pack::p4_fixed_bit(ctx.arena_mut(), 16.into(), 0.into())?;
    let value_ctx = rel::lvalue_write_dot_local(
        ctx,
        value_ctx,
        value_arch,
        "standard_metadata",
        "mcast_grp",
        value_mcast_grp,
    )?;
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
    Ok((value_ctx, value_arch, value_call_result))
}

/// Hashes `data` with `algo` into `result`, in `[base, base + max - 1]`.
///
/// `max == 0` always yields `base`; `O`, `T`, and `M` are `bit<W>` types
/// and `D` a tuple of bit-fields or varbits.
pub fn hash<Interp, Iface, Ext>(
    ctx: &mut RunnerContext<'_, Interp, Iface, Ext>,
    value_ctx: Value,
    value_arch: Value,
) -> Result<(Value, Value, Value), Interp::Error>
where
    Iface: Interface,
    Ext: Extern,
    Interp: Interpreter<Iface, Ext>,
{
    let value_base = func::find_var_e_local(ctx, value_ctx, "base")?;
    let int_base = unpack::p4_fixed_bit(ctx.arena(), &value_base)?.1;
    let value_max = func::find_var_e_local(ctx, value_ctx, "max")?;
    let int_max = unpack::p4_fixed_bit(ctx.arena(), &value_max)?.1;
    let int = compute_checksum(ctx, value_ctx, None)?;
    // The source simulator uses max - base as the range divisor
    let int = adjust(&int_base, &int_max, &int)?;
    // Cast the arbitrary-precision result to the output type `O`
    let value_typ = func::find_type_e_local(ctx, value_ctx, "O")?;
    let value_result = pack::p4_arbitrary_int(ctx.arena_mut(), int)?;
    let value_result = func::cast_op(ctx, value_typ, value_result)?;
    let value_ctx =
        rel::lvalue_write_var_local(ctx, value_ctx, value_arch, "result", value_result)?;
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
    Ok((value_ctx, value_arch, value_call_result))
}

/// Folds `int` into `[base, rmax)`, or returns `base` when `rmax` is zero.
pub fn adjust(base: &BigInt, rmax: &BigInt, int: &BigInt) -> Result<BigInt, ExternError> {
    // max = 0: the result is always base
    if rmax.is_zero() {
        return Ok(base.clone());
    }
    let int_range = rmax - base;
    // The divisor max - base must be positive
    if int_range <= BigInt::zero() {
        return Err(ExternError::Failure("hash range divisor must be positive".to_owned()));
    }
    Ok(remainder(int, &int_range) + base)
}

/// Hashes the `data` tuple, plus the unparsed payload if given, with `algo`.
fn compute_checksum<Interp, Iface, Ext>(
    ctx: &mut RunnerContext<'_, Interp, Iface, Ext>,
    value_ctx: Value,
    payload: Option<&PacketIn>,
) -> Result<BigInt, Interp::Error>
where
    Iface: Interface,
    Ext: Extern,
    Interp: Interpreter<Iface, Ext>,
{
    let value_data = func::find_var_e_local(ctx, value_ctx, "data")?;
    let mut values = unpack::p4_tuple(ctx.arena(), &value_data)?;
    // The payload is appended as 8-bit values
    if let Some(packet_in) = payload {
        for byte in packet_in.payload_bytes()? {
            values.push(pack::p4_fixed_bit(ctx.arena_mut(), 8.into(), byte)?);
        }
    }
    let value_algo = func::find_var_e_local(ctx, value_ctx, "algo")?;
    let (id_enum, id_field) = unpack::p4_enum(ctx.arena(), &value_algo)?;
    // Only a `HashAlgorithm` enumerator selects the algorithm
    if id_enum != "HashAlgorithm" {
        return Err(ExternError::Failure(format!(
            "invalid HashAlgorithm enum value: {id_enum}.{id_field}"
        ))
        .into());
    }
    Ok(checksum::compute_checksum(&id_field, None, ctx.arena(), &values)?)
}

/// Shared body of `verify_checksum` and its `_with_payload` variant.
fn do_verify_checksum<Interp, Iface, Ext>(
    ctx: &mut RunnerContext<'_, Interp, Iface, Ext>,
    value_ctx: Value,
    value_arch: Value,
    payload: Option<&PacketIn>,
) -> Result<(Value, Value, Value), Interp::Error>
where
    Iface: Interface,
    Ext: Extern,
    Interp: Interpreter<Iface, Ext>,
{
    let value_condition = func::find_var_e_local(ctx, value_ctx, "condition")?;
    // A false condition skips the checksum
    if !unpack::p4_bool(ctx.arena(), &value_condition)? {
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
        return Ok((value_ctx, value_arch, value_call_result));
    }
    let value_checksum = func::find_var_e_local(ctx, value_ctx, "checksum")?;
    let int_expect = unpack::p4_fixed_bit(ctx.arena(), &value_checksum)?.1;
    let int_actual = compute_checksum(ctx, value_ctx, payload)?;
    // A mismatch sets `checksum_error` to 1
    let value_ctx = if int_expect == int_actual {
        value_ctx
    } else {
        let value_error = pack::p4_fixed_bit(ctx.arena_mut(), 1.into(), 1.into())?;
        rel::lvalue_write_dot_global(
            ctx,
            value_ctx,
            value_arch,
            "standard_metadata",
            "checksum_error",
            value_error,
        )?
    };
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
    Ok((value_ctx, value_arch, value_call_result))
}

/// Compares the checksum of `data` with `checksum`.
///
/// A mismatch sets `standard_metadata.checksum_error` to 1 before ingress;
/// `T` is a tuple of `bit<W>`, `int<W>`, or `varbit<W>` fields
/// and `O` a `bit<X>` type; `algo` must be a compile-time constant.
/// Only supported in the VerifyChecksum control.
pub fn verify_checksum<Interp, Iface, Ext>(
    ctx: &mut RunnerContext<'_, Interp, Iface, Ext>,
    value_ctx: Value,
    value_arch: Value,
) -> Result<(Value, Value, Value), Interp::Error>
where
    Iface: Interface,
    Ext: Extern,
    Interp: Interpreter<Iface, Ext>,
{
    do_verify_checksum(ctx, value_ctx, value_arch, None)
}

/// `verify_checksum` over `data` plus the packet's unparsed payload.
///
/// Only supported in the VerifyChecksum control.
pub fn verify_checksum_with_payload<Interp, Iface, Ext>(
    ctx: &mut RunnerContext<'_, Interp, Iface, Ext>,
    value_ctx: Value,
    value_arch: Value,
    packet_in: &PacketIn,
) -> Result<(Value, Value, Value), Interp::Error>
where
    Iface: Interface,
    Ext: Extern,
    Interp: Interpreter<Iface, Ext>,
{
    do_verify_checksum(ctx, value_ctx, value_arch, Some(packet_in))
}

/// Shared body of `update_checksum` and its `_with_payload` variant.
fn do_update_checksum<Interp, Iface, Ext>(
    ctx: &mut RunnerContext<'_, Interp, Iface, Ext>,
    value_ctx: Value,
    value_arch: Value,
    payload: Option<&PacketIn>,
) -> Result<(Value, Value, Value), Interp::Error>
where
    Iface: Interface,
    Ext: Extern,
    Interp: Interpreter<Iface, Ext>,
{
    let value_condition = func::find_var_e_local(ctx, value_ctx, "condition")?;
    // A false condition skips the checksum
    if !unpack::p4_bool(ctx.arena(), &value_condition)? {
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
        return Ok((value_ctx, value_arch, value_call_result));
    }
    let int = compute_checksum(ctx, value_ctx, payload)?;
    // Cast the arbitrary-precision result to the output type `O`
    let value_typ = func::find_type_e_local(ctx, value_ctx, "O")?;
    let value_checksum = pack::p4_arbitrary_int(ctx.arena_mut(), int)?;
    let value_checksum = func::cast_op(ctx, value_typ, value_checksum)?;
    let value_ctx =
        rel::lvalue_write_var_local(ctx, value_ctx, value_arch, "checksum", value_checksum)?;
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
    Ok((value_ctx, value_arch, value_call_result))
}

/// Computes the checksum of `data` into `checksum`.
///
/// A false `condition` leaves `checksum` unchanged;
/// the type constraints are those of `verify_checksum`.
/// Only supported in the ComputeChecksum control.
pub fn update_checksum<Interp, Iface, Ext>(
    ctx: &mut RunnerContext<'_, Interp, Iface, Ext>,
    value_ctx: Value,
    value_arch: Value,
) -> Result<(Value, Value, Value), Interp::Error>
where
    Iface: Interface,
    Ext: Extern,
    Interp: Interpreter<Iface, Ext>,
{
    do_update_checksum(ctx, value_ctx, value_arch, None)
}

/// `update_checksum` over `data` plus the packet's unparsed payload.
///
/// Only supported in the ComputeChecksum control.
pub fn update_checksum_with_payload<Interp, Iface, Ext>(
    ctx: &mut RunnerContext<'_, Interp, Iface, Ext>,
    value_ctx: Value,
    value_arch: Value,
    packet_in: &PacketIn,
) -> Result<(Value, Value, Value), Interp::Error>
where
    Iface: Interface,
    Ext: Extern,
    Interp: Interpreter<Iface, Ext>,
{
    do_update_checksum(ctx, value_ctx, value_arch, Some(packet_in))
}

/// Requests a resubmit: the packet is parsed again from its original bytes.
///
/// Only `standard_metadata.instance_type` and the `@field_list(index)`
/// metadata fields survive; the last call in an ingress pass wins.
/// Only supported in the ingress control.
pub fn resubmit_preserving_field_list<Interp, Iface>(
    ctx: &mut RunnerContext<'_, Interp, Iface, V1Model>,
    value_ctx: Value,
    value_arch: Value,
) -> Result<(Value, Value, Value), Interp::Error>
where
    Iface: Interface,
    Interp: Interpreter<Iface, V1Model>,
{
    let value_idx = func::find_var_e_local(ctx, value_ctx, "index")?;
    let idx = usize::try_from(&unpack::p4_fixed_bit(ctx.arena(), &value_idx)?.1)
        .map_err(ExternError::from)?;
    // The scheduler acts on the request after the control returns
    let mut arch = pipe::find_arch_state(ctx, value_arch)?;
    arch.action.resubmit_opt = Some(idx);
    let value_arch = pipe::update_arch_state(ctx, value_arch, &arch)?;
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
    Ok((value_ctx, value_arch, value_call_result))
}

/// Requests a recirculate: the deparsed packet is parsed again.
///
/// Recirculated packets are told apart by `standard_metadata.instance_type`;
/// the `@field_list(index)` metadata fields are preserved
/// and the last call in an egress pass wins.
/// Only supported in the egress control.
pub fn recirculate_preserving_field_list<Interp, Iface>(
    ctx: &mut RunnerContext<'_, Interp, Iface, V1Model>,
    value_ctx: Value,
    value_arch: Value,
) -> Result<(Value, Value, Value), Interp::Error>
where
    Iface: Interface,
    Interp: Interpreter<Iface, V1Model>,
{
    let value_idx = func::find_var_e_local(ctx, value_ctx, "index")?;
    let idx = usize::try_from(&unpack::p4_fixed_bit(ctx.arena(), &value_idx)?.1)
        .map_err(ExternError::from)?;
    // The scheduler acts on the request after the control returns
    let mut arch = pipe::find_arch_state(ctx, value_arch)?;
    arch.action.recirculate_opt = Some(idx);
    let value_arch = pipe::update_arch_state(ctx, value_arch, &arch)?;
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
    Ok((value_ctx, value_arch, value_call_result))
}

/// Requests a clone of the packet through mirror `session`.
///
/// Each clone later runs egress on its own; the original continues.
/// The control plane must configure the session, else no clone is made.
/// `type` is `I2E` in ingress and `E2E` in egress;
/// the `@field_list(index)` metadata fields are preserved
/// and the last call in a pass wins.
pub fn clone_preserving_field_list<Interp, Iface>(
    ctx: &mut RunnerContext<'_, Interp, Iface, V1Model>,
    value_ctx: Value,
    value_arch: Value,
) -> Result<(Value, Value, Value), Interp::Error>
where
    Iface: Interface,
    Interp: Interpreter<Iface, V1Model>,
{
    // The scheduler acts on the request after the control returns
    let mut arch = pipe::find_arch_state(ctx, value_arch)?;
    let value_type = func::find_var_e_local(ctx, value_ctx, "type")?;
    let value_session = func::find_var_e_local(ctx, value_ctx, "session")?;
    let value_idx = func::find_var_e_local(ctx, value_ctx, "index")?;
    arch.action.clone_opt =
        Some(CloneInfo::new(ctx.arena(), &value_type, &value_session, &value_idx)?);
    let value_arch = pipe::update_arch_state(ctx, value_arch, &arch)?;
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
    Ok((value_ctx, value_arch, value_call_result))
}

/// Prints `msg` to standard output.
pub fn log_msg<Interp, Iface, Ext>(
    ctx: &mut RunnerContext<'_, Interp, Iface, Ext>,
    value_ctx: Value,
    value_arch: Value,
) -> Result<(Value, Value, Value), Interp::Error>
where
    Iface: Interface,
    Ext: Extern,
    Interp: Interpreter<Iface, Ext>,
{
    let value_msg = func::find_var_e_local(ctx, value_ctx, "msg")?;
    let msg = unpack::p4_string(ctx.arena(), &value_msg)?;
    println!("{msg}");
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
    Ok((value_ctx, value_arch, value_call_result))
}

/// Expands `{}` holes in `fmt` with `args`; `{{` and `}}` are literal braces.
pub fn format_braces(arena: &ValueArena, fmt: &str, args: &[Value]) -> Result<String, ExternError> {
    let mut chars = fmt.chars().peekable();
    let mut args = args.iter();
    let mut text = String::with_capacity(fmt.len());
    while let Some(char) = chars.next() {
        match (char, chars.peek().copied()) {
            // Doubled braces escape themselves
            ('{', Some('{')) | ('}', Some('}')) => {
                chars.next();
                text.push(char);
            }
            // `{}` consumes the next argument
            ('{', Some('}')) => {
                chars.next();
                let value = args.next().ok_or_else(|| {
                    ExternError::Failure(
                        "not enough arguments for format string in log_msg".to_owned(),
                    )
                })?;
                text.push_str(&arena.to_string(value));
            }
            // Anything else is copied
            _ => text.push(char),
        }
    }
    // Every argument must be consumed
    if args.next().is_some() {
        return Err(ExternError::Failure(
            "too many arguments for format string in log_msg".to_owned(),
        ));
    }
    Ok(text)
}

/// Prints `msg` with each `{}` replaced by the next element of `data`.
pub fn log_msg_format<Interp, Iface, Ext>(
    ctx: &mut RunnerContext<'_, Interp, Iface, Ext>,
    value_ctx: Value,
    value_arch: Value,
) -> Result<(Value, Value, Value), Interp::Error>
where
    Iface: Interface,
    Ext: Extern,
    Interp: Interpreter<Iface, Ext>,
{
    let value_msg = func::find_var_e_local(ctx, value_ctx, "msg")?;
    let msg = unpack::p4_string(ctx.arena(), &value_msg)?;
    let value_data = func::find_var_e_local(ctx, value_ctx, "data")?;
    let values = unpack::p4_tuple(ctx.arena(), &value_data)?;
    let text = format_braces(ctx.arena(), &msg, &values)?;
    println!("{text}");
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
    Ok((value_ctx, value_arch, value_call_result))
}
