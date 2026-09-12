//! Helpers for invoking relations in the spec

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

pub fn lvalue_read_dot_global<Interp, Iface, Exn>(
    ctx: &mut RunnerContext<'_, Interp, Iface, Exn>,
    value_ctx: Value,
    value_arch: Value,
    name: &str,
    member: &str,
) -> Result<Value, Interp::Error>
where
    Iface: Interface,
    Exn: Extern,
    Interp: Interpreter<Iface, Exn>,
{
    use crate::lang::{common::source::Span, data::value::make};
    let value_cursor =
        make::case_shaped_(ctx.arena_mut(), "GLOBAL", vec![], "cursor", Span::default())
            .map_err(ExternError::from)?;
    let value_base = super::func::bare_name(ctx.arena_mut(), name)?;
    let value_member = make::text(ctx.arena_mut(), member.to_owned(), Span::default())
        .map_err(ExternError::from)?;
    let value_ref = make::case_shaped_(
        ctx.arena_mut(),
        "storageReference '.' nameIR",
        vec![value_base, value_member],
        "storageReference",
        Span::default(),
    )
    .map_err(ExternError::from)?;
    let values = ctx.call_rel(
        "Lvalue_read",
        &[value_cursor, value_ctx, value_arch, value_ref],
    )?;
    Ok(*get::one(&values).map_err(ExternError::from)?)
}

pub fn lvalue_write_dot_global<Interp, Iface, Exn>(
    ctx: &mut RunnerContext<'_, Interp, Iface, Exn>,
    value_ctx: Value,
    value_arch: Value,
    name: &str,
    member: &str,
    value: Value,
) -> Result<Value, Interp::Error>
where
    Iface: Interface,
    Exn: Extern,
    Interp: Interpreter<Iface, Exn>,
{
    use crate::lang::{common::source::Span, data::value::make};
    let value_cursor =
        make::case_shaped_(ctx.arena_mut(), "GLOBAL", vec![], "cursor", Span::default())
            .map_err(ExternError::from)?;
    let value_base = super::func::bare_name(ctx.arena_mut(), name)?;
    let value_member = make::text(ctx.arena_mut(), member.to_owned(), Span::default())
        .map_err(ExternError::from)?;
    let value_ref = make::case_shaped_(
        ctx.arena_mut(),
        "storageReference '.' nameIR",
        vec![value_base, value_member],
        "storageReference",
        Span::default(),
    )
    .map_err(ExternError::from)?;
    let values = ctx.call_rel(
        "Lvalue_write",
        &[value_cursor, value_ctx, value_arch, value_ref, value],
    )?;
    Ok(*get::one(&values).map_err(ExternError::from)?)
}

pub fn psa_ingress_init_packet_in<Interp, Iface, Exn>(
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
        "PSA_ingress_init_packet_in",
        &[value_ctx, value_arch, value_packet],
    )?;
    let (value_ctx, value_arch) = get::two(&values).map_err(ExternError::from)?;
    Ok(StateResult {
        value_ctx: *value_ctx,
        value_arch: *value_arch,
    })
}

pub fn psa_ingress_init_packet_out<Interp, Iface, Exn>(
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
        "PSA_ingress_init_packet_out",
        &[value_ctx, value_arch, value_packet],
    )?;
    let (value_ctx, value_arch) = get::two(&values).map_err(ExternError::from)?;
    Ok(StateResult {
        value_ctx: *value_ctx,
        value_arch: *value_arch,
    })
}

pub fn psa_ingress_parser<Interp, Iface, Exn>(
    ctx: &mut RunnerContext<'_, Interp, Iface, Exn>,
    value_ctx: Value,
    value_arch: Value,
) -> Result<CallResult, Interp::Error>
where
    Iface: Interface,
    Exn: Extern,
    Interp: Interpreter<Iface, Exn>,
{
    let values = ctx.call_rel("PSA_ingress_parser", &[value_ctx, value_arch])?;
    let (value_ctx, value_arch, value_call_result) =
        get::three(&values).map_err(ExternError::from)?;
    Ok(CallResult {
        value_ctx: *value_ctx,
        value_arch: *value_arch,
        value_call_result: *value_call_result,
    })
}

pub fn psa_ingress<Interp, Iface, Exn>(
    ctx: &mut RunnerContext<'_, Interp, Iface, Exn>,
    value_ctx: Value,
    value_arch: Value,
) -> Result<CallResult, Interp::Error>
where
    Iface: Interface,
    Exn: Extern,
    Interp: Interpreter<Iface, Exn>,
{
    let values = ctx.call_rel("PSA_ingress", &[value_ctx, value_arch])?;
    let (value_ctx, value_arch, value_call_result) =
        get::three(&values).map_err(ExternError::from)?;
    Ok(CallResult {
        value_ctx: *value_ctx,
        value_arch: *value_arch,
        value_call_result: *value_call_result,
    })
}

pub fn psa_ingress_deparser<Interp, Iface, Exn>(
    ctx: &mut RunnerContext<'_, Interp, Iface, Exn>,
    value_ctx: Value,
    value_arch: Value,
) -> Result<CallResult, Interp::Error>
where
    Iface: Interface,
    Exn: Extern,
    Interp: Interpreter<Iface, Exn>,
{
    let values = ctx.call_rel("PSA_ingress_deparser", &[value_ctx, value_arch])?;
    let (value_ctx, value_arch, value_call_result) =
        get::three(&values).map_err(ExternError::from)?;
    Ok(CallResult {
        value_ctx: *value_ctx,
        value_arch: *value_arch,
        value_call_result: *value_call_result,
    })
}

pub fn psa_ingress_init_globals<Interp, Iface, Exn>(
    ctx: &mut RunnerContext<'_, Interp, Iface, Exn>,
    value_ctx: Value,
    value_arch: Value,
    port: i64,
) -> Result<Value, Interp::Error>
where
    Iface: Interface,
    Exn: Extern,
    Interp: Interpreter<Iface, Exn>,
{
    let value_port = crate::lang::data::value::make::int(
        ctx.arena_mut(),
        port.into(),
        crate::lang::common::source::Span::default(),
    )
    .map_err(ExternError::from)?;
    let values = ctx.call_rel(
        "PSA_ingress_init_globals",
        &[value_ctx, value_arch, value_port],
    )?;
    Ok(*get::one(&values).map_err(ExternError::from)?)
}

pub fn psa_ingress_init_metadata<Interp, Iface, Exn>(
    ctx: &mut RunnerContext<'_, Interp, Iface, Exn>,
    value_ctx: Value,
    value_arch: Value,
    port: i64,
    path: &str,
) -> Result<Value, Interp::Error>
where
    Iface: Interface,
    Exn: Extern,
    Interp: Interpreter<Iface, Exn>,
{
    use crate::lang::{common::source::Span, data::value::make};
    let value_port =
        make::int(ctx.arena_mut(), port.into(), Span::default()).map_err(ExternError::from)?;
    let value_path =
        make::text(ctx.arena_mut(), path.to_owned(), Span::default()).map_err(ExternError::from)?;
    let values = ctx.call_rel(
        "PSA_ingress_init_metadata",
        &[value_ctx, value_arch, value_port, value_path],
    )?;
    Ok(*get::one(&values).map_err(ExternError::from)?)
}

pub fn psa_egress_init_packet_in<Interp, Iface, Exn>(
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
        "PSA_egress_init_packet_in",
        &[value_ctx, value_arch, value_packet],
    )?;
    let (value_ctx, value_arch) = get::two(&values).map_err(ExternError::from)?;
    Ok(StateResult {
        value_ctx: *value_ctx,
        value_arch: *value_arch,
    })
}

pub fn psa_egress_init_packet_out<Interp, Iface, Exn>(
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
        "PSA_egress_init_packet_out",
        &[value_ctx, value_arch, value_packet],
    )?;
    let (value_ctx, value_arch) = get::two(&values).map_err(ExternError::from)?;
    Ok(StateResult {
        value_ctx: *value_ctx,
        value_arch: *value_arch,
    })
}

pub fn psa_egress_parser<Interp, Iface, Exn>(
    ctx: &mut RunnerContext<'_, Interp, Iface, Exn>,
    value_ctx: Value,
    value_arch: Value,
) -> Result<CallResult, Interp::Error>
where
    Iface: Interface,
    Exn: Extern,
    Interp: Interpreter<Iface, Exn>,
{
    let values = ctx.call_rel("PSA_egress_parser", &[value_ctx, value_arch])?;
    let (value_ctx, value_arch, value_call_result) =
        get::three(&values).map_err(ExternError::from)?;
    Ok(CallResult {
        value_ctx: *value_ctx,
        value_arch: *value_arch,
        value_call_result: *value_call_result,
    })
}

pub fn psa_egress<Interp, Iface, Exn>(
    ctx: &mut RunnerContext<'_, Interp, Iface, Exn>,
    value_ctx: Value,
    value_arch: Value,
) -> Result<CallResult, Interp::Error>
where
    Iface: Interface,
    Exn: Extern,
    Interp: Interpreter<Iface, Exn>,
{
    let values = ctx.call_rel("PSA_egress", &[value_ctx, value_arch])?;
    let (value_ctx, value_arch, value_call_result) =
        get::three(&values).map_err(ExternError::from)?;
    Ok(CallResult {
        value_ctx: *value_ctx,
        value_arch: *value_arch,
        value_call_result: *value_call_result,
    })
}

pub fn psa_egress_deparser<Interp, Iface, Exn>(
    ctx: &mut RunnerContext<'_, Interp, Iface, Exn>,
    value_ctx: Value,
    value_arch: Value,
) -> Result<CallResult, Interp::Error>
where
    Iface: Interface,
    Exn: Extern,
    Interp: Interpreter<Iface, Exn>,
{
    let values = ctx.call_rel("PSA_egress_deparser", &[value_ctx, value_arch])?;
    let (value_ctx, value_arch, value_call_result) =
        get::three(&values).map_err(ExternError::from)?;
    Ok(CallResult {
        value_ctx: *value_ctx,
        value_arch: *value_arch,
        value_call_result: *value_call_result,
    })
}

pub fn psa_egress_init_globals<Interp, Iface, Exn>(
    ctx: &mut RunnerContext<'_, Interp, Iface, Exn>,
    value_ctx: Value,
    value_arch: Value,
    port: i64,
) -> Result<Value, Interp::Error>
where
    Iface: Interface,
    Exn: Extern,
    Interp: Interpreter<Iface, Exn>,
{
    let value_port = crate::lang::data::value::make::int(
        ctx.arena_mut(),
        port.into(),
        crate::lang::common::source::Span::default(),
    )
    .map_err(ExternError::from)?;
    let values = ctx.call_rel(
        "PSA_egress_init_globals",
        &[value_ctx, value_arch, value_port],
    )?;
    Ok(*get::one(&values).map_err(ExternError::from)?)
}

pub fn psa_egress_init_metadata<Interp, Iface, Exn>(
    ctx: &mut RunnerContext<'_, Interp, Iface, Exn>,
    value_ctx: Value,
    value_arch: Value,
    port: i64,
    path: &str,
    cos: i64,
    instance: i64,
) -> Result<Value, Interp::Error>
where
    Iface: Interface,
    Exn: Extern,
    Interp: Interpreter<Iface, Exn>,
{
    use crate::lang::{common::source::Span, data::value::make};
    let value_port =
        make::int(ctx.arena_mut(), port.into(), Span::default()).map_err(ExternError::from)?;
    let value_path =
        make::text(ctx.arena_mut(), path.to_owned(), Span::default()).map_err(ExternError::from)?;
    let value_cos =
        make::int(ctx.arena_mut(), cos.into(), Span::default()).map_err(ExternError::from)?;
    let value_instance =
        make::int(ctx.arena_mut(), instance.into(), Span::default()).map_err(ExternError::from)?;
    let values = ctx.call_rel(
        "PSA_egress_init_metadata",
        &[
            value_ctx,
            value_arch,
            value_port,
            value_path,
            value_cos,
            value_instance,
        ],
    )?;
    Ok(*get::one(&values).map_err(ExternError::from)?)
}
