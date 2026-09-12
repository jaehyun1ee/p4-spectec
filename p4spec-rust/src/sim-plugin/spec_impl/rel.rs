use crate::{
    lang::data::value::{Value, get},
    runner::{Extern, ExternError, Interface, Interpreter, RunnerContext},
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CallResult {
    pub value_ctx: Value,
    pub value_arch: Value,
    pub value_call_result: Value,
}

pub fn lvalue_write_var_local<Interp, Iface, Exn>(
    ctx: &mut RunnerContext<'_, Interp, Iface, Exn>,
    value_ctx: Value,
    value_arch: Value,
    name: &str,
    value: Value,
) -> Result<Value, Interp::Error>
where
    Iface: Interface,
    Exn: Extern,
    Interp: Interpreter<Iface, Exn>,
{
    let value_cursor = super::func::local_cursor(ctx.arena_mut())?;
    let value_name = super::func::bare_name(ctx.arena_mut(), name)?;
    let values = ctx.call_rel(
        "Lvalue_write",
        &[value_cursor, value_ctx, value_arch, value_name, value],
    )?;
    Ok(*get::one(&values).map_err(ExternError::from)?)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct StateResult {
    pub value_ctx: Value,
    pub value_arch: Value,
}

pub fn ebpf_init_packet_in<Interp, Iface, Exn>(
    ctx: &mut RunnerContext<'_, Interp, Iface, Exn>,
    value_ctx: Value,
    value_arch: Value,
    value_packet: Value,
) -> Result<StateResult, Interp::Error>
where
    Iface: Interface,
    Exn: Extern,
    Interp: Interpreter<Iface, Exn>,
{
    let values = ctx.call_rel(
        "EBPF_init_packet_in",
        &[value_ctx, value_arch, value_packet],
    )?;
    let (value_ctx, value_arch) = get::two(&values).map_err(ExternError::from)?;
    Ok(StateResult {
        value_ctx: *value_ctx,
        value_arch: *value_arch,
    })
}

pub fn ebpf_init_globals<Interp, Iface, Exn>(
    ctx: &mut RunnerContext<'_, Interp, Iface, Exn>,
    value_ctx: Value,
    value_arch: Value,
) -> Result<Value, Interp::Error>
where
    Iface: Interface,
    Exn: Extern,
    Interp: Interpreter<Iface, Exn>,
{
    let values = ctx.call_rel("EBPF_init_globals", &[value_ctx, value_arch])?;
    Ok(*get::one(&values).map_err(ExternError::from)?)
}

pub fn lvalue_read_var_global<Interp, Iface, Exn>(
    ctx: &mut RunnerContext<'_, Interp, Iface, Exn>,
    value_ctx: Value,
    value_arch: Value,
    name: &str,
) -> Result<Value, Interp::Error>
where
    Iface: Interface,
    Exn: Extern,
    Interp: Interpreter<Iface, Exn>,
{
    let value_cursor = crate::lang::data::value::make::case_shaped_(
        ctx.arena_mut(),
        "GLOBAL",
        vec![],
        "cursor",
        crate::lang::common::source::Span::default(),
    )
    .map_err(ExternError::from)?;
    let value_name = super::func::bare_name(ctx.arena_mut(), name)?;
    let values = ctx.call_rel(
        "Lvalue_read",
        &[value_cursor, value_ctx, value_arch, value_name],
    )?;
    Ok(*get::one(&values).map_err(ExternError::from)?)
}

pub fn ebpf_parse<Interp, Iface, Exn>(
    ctx: &mut RunnerContext<'_, Interp, Iface, Exn>,
    value_ctx: Value,
    value_arch: Value,
) -> Result<CallResult, Interp::Error>
where
    Iface: Interface,
    Exn: Extern,
    Interp: Interpreter<Iface, Exn>,
{
    let values = ctx.call_rel("EBPF_parse", &[value_ctx, value_arch])?;
    let (value_ctx, value_arch, value_call_result) =
        get::three(&values).map_err(ExternError::from)?;
    Ok(CallResult {
        value_ctx: *value_ctx,
        value_arch: *value_arch,
        value_call_result: *value_call_result,
    })
}

pub fn ebpf_filter<Interp, Iface, Exn>(
    ctx: &mut RunnerContext<'_, Interp, Iface, Exn>,
    value_ctx: Value,
    value_arch: Value,
) -> Result<CallResult, Interp::Error>
where
    Iface: Interface,
    Exn: Extern,
    Interp: Interpreter<Iface, Exn>,
{
    let values = ctx.call_rel("EBPF_filter", &[value_ctx, value_arch])?;
    let (value_ctx, value_arch, value_call_result) =
        get::three(&values).map_err(ExternError::from)?;
    Ok(CallResult {
        value_ctx: *value_ctx,
        value_arch: *value_arch,
        value_call_result: *value_call_result,
    })
}
